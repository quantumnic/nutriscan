mod api;
mod analyzer;
mod cache;
mod daily;
mod display;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "nutriscan", version, about = "Offline food analyzer using Open Food Facts")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to cache database
    #[arg(long, default_value = "~/.nutriscan/cache.db")]
    cache_dir: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a product by name or barcode
    Scan {
        /// Product name or barcode
        query: String,
    },
    /// Show additive warnings for a product
    Warn {
        /// Product name or barcode
        query: String,
    },
    /// Compare two products
    Compare {
        /// First product
        a: String,
        /// Second product
        b: String,
    },
    /// Update local cache from API
    Update {
        /// Search term to cache
        query: String,
        /// Number of products to fetch
        #[arg(short, long, default_value_t = 50)]
        limit: u32,
    },
    /// Look up a product by barcode
    Barcode {
        /// EAN/UPC barcode number
        code: String,
    },
    /// Search offline cache only (no network)
    Offline {
        /// Search term
        query: String,
    },
    /// Show cache statistics
    Stats,
    /// Show recently scanned/cached products
    History {
        /// Number of recent products to show
        #[arg(short, long, default_value_t = 10)]
        limit: u32,
    },
    /// Export cache to JSON file
    Export {
        /// Output file path
        #[arg(short, long, default_value = "nutriscan-export.json")]
        output: String,
    },
    /// Purge stale cache entries older than N days
    Purge {
        /// Maximum age in days
        #[arg(short, long, default_value_t = 90)]
        days: u32,
    },
    /// Log a product to daily intake tracker
    Log {
        /// Product name or barcode
        query: String,
        /// Number of servings (each = 100g)
        #[arg(short, long, default_value_t = 1.0)]
        servings: f64,
    },
    /// Show today's intake summary (or a specific date)
    Daily {
        /// Show last 7 days instead of a single day
        #[arg(short, long)]
        week: bool,
        /// Date in YYYY-MM-DD format (defaults to today)
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Clear daily log for a date
    ClearDay {
        /// Date in YYYY-MM-DD format (defaults to today)
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Refresh stale cache entries from the API
    Refresh {
        /// Maximum age in days before considering stale
        #[arg(short, long, default_value_t = 30)]
        days: u32,
        /// Maximum number of entries to refresh
        #[arg(short, long, default_value_t = 20)]
        limit: u32,
    },
}

fn resolve_cache_path(raw: &str) -> PathBuf {
    if raw.starts_with('~') {
        if let Some(home) = dirs_fallback() {
            return PathBuf::from(raw.replacen('~', &home, 1));
        }
    }
    PathBuf::from(raw)
}

fn dirs_fallback() -> Option<String> {
    std::env::var("HOME").ok()
}

fn today() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Simple date calculation (UTC)
    let days = now / 86400;
    let (y, m, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}


fn date_minus_days(date: &str, n: u64) -> String {
    let parts: Vec<u64> = date.split('-').filter_map(|s| s.parse().ok()).collect();
    if parts.len() != 3 { return date.to_string(); }
    let (y, m, d) = (parts[0], parts[1], parts[2]);
    // Convert to days since epoch, subtract, convert back
    let days = ymd_to_days(y, m, d).saturating_sub(n);
    let (y2, m2, d2) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", y2, m2, d2)
}

fn ymd_to_days(y: u64, m: u64, d: u64) -> u64 {
    // Inverse of days_to_ymd (Howard Hinnant algorithm)
    let y = if m <= 2 { y - 1 } else { y };
    let era = y / 400;
    let yoe = y - era * 400;
    let m_adj = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * m_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let cache_path = resolve_cache_path(&cli.cache_dir);

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = cache::Cache::open(&cache_path)?;
    let api = api::OpenFoodFactsApi::new();

    // Daily log lives next to the cache
    let daily_path = cache_path.with_extension("daily.db");
    let daily_log = daily::DailyLog::open(&daily_path)?;

    match cli.command {
        Commands::Scan { query } => {
            let cached = db.search(&query)?;
            if !cached.is_empty() {
                println!("(from cache)");
                for p in &cached {
                    let a = analyzer::analyze(p);
                    display::print_analysis(&a);
                }
            } else {
                println!("Searching Open Food Facts...");
                let products = api.search(&query, 5).await?;
                if products.is_empty() {
                    println!("No products found for '{}'.", query);
                } else {
                    for p in &products {
                        db.upsert(p)?;
                        let a = analyzer::analyze(p);
                        display::print_analysis(&a);
                    }
                }
            }
        }
        Commands::Warn { query } => {
            let cached = db.search(&query)?;
            let products = if !cached.is_empty() {
                cached
            } else {
                let fetched = api.search(&query, 5).await?;
                for p in &fetched {
                    db.upsert(p)?;
                }
                fetched
            };
            if products.is_empty() {
                println!("No products found for '{}'.", query);
            } else {
                for p in &products {
                    let a = analyzer::analyze(p);
                    display::print_warnings(&a.warnings, &a.product_name);
                }
            }
        }
        Commands::Compare { a, b } => {
            let pa = find_product(&db, &api, &a).await?;
            let pb = find_product(&db, &api, &b).await?;
            match (pa, pb) {
                (Some(pa), Some(pb)) => {
                    let diffs = analyzer::compare_products(&pa, &pb);
                    display::print_comparison(&pa, &pb, &diffs);
                }
                (None, _) => println!("Product '{}' not found.", a),
                (_, None) => println!("Product '{}' not found.", b),
            }
        }
        Commands::Update { query, limit } => {
            println!("Fetching up to {} products for '{}'...", limit, query);
            let products = api.search(&query, limit).await?;
            let count = products.len();
            for p in &products {
                db.upsert(p)?;
            }
            println!("Cached {} products.", count);
        }
        Commands::Barcode { code } => {
            if let Some(p) = db.get_by_code(&code)? {
                println!("(from cache)");
                let a = analyzer::analyze(&p);
                display::print_analysis(&a);
            } else {
                println!("Looking up barcode {}...", code);
                match api.get_by_barcode(&code).await? {
                    Some(p) => {
                        db.upsert(&p)?;
                        let a = analyzer::analyze(&p);
                        display::print_analysis(&a);
                    }
                    None => println!("No product found for barcode '{}'.", code),
                }
            }
        }
        Commands::Offline { query } => {
            let cached = db.search(&query)?;
            if cached.is_empty() {
                println!("No cached products matching '{}'. Use 'update' to populate the cache.", query);
            } else {
                println!("({} result(s) from cache)", cached.len());
                for p in &cached {
                    let a = analyzer::analyze(p);
                    display::print_analysis(&a);
                }
            }
        }
        Commands::Stats => {
            let (bytes, count) = db.size_info()?;
            let kb = bytes as f64 / 1024.0;
            println!("Cache: {} products stored ({:.1} KB)", count, kb);
        }
        Commands::History { limit } => {
            let products = db.recent(limit)?;
            if products.is_empty() {
                println!("No cached products yet. Use 'scan' or 'update' to populate.");
            } else {
                println!("Last {} cached product(s):", products.len());
                for (i, p) in products.iter().enumerate() {
                    let name = p.product_name.as_deref().unwrap_or("Unknown");
                    let brand = p.brands.as_deref().unwrap_or("");
                    let grade = p.nutriscore_grade.as_deref().unwrap_or("?");
                    println!("  {}. {} ({}) — Nutri-Score {}", i + 1, name, brand, grade.to_uppercase());
                }
            }
        }
        Commands::Export { output } => {
            let json = db.export_json()?;
            std::fs::write(&output, &json)?;
            let count = db.count()?;
            println!("Exported {} products to {}", count, output);
        }
        Commands::Purge { days } => {
            let evicted = db.evict_stale(days)?;
            println!("Purged {} stale entries (older than {} days).", evicted, days);
        }
        Commands::Log { query, servings } => {
            let product = find_product(&db, &api, &query).await?;
            match product {
                Some(p) => {
                    let date = today();
                    let name = p.product_name.clone().unwrap_or_else(|| "Unknown".into());
                    daily_log.log_product(&date, &p, servings)?;
                    db.upsert(&p)?;
                    println!("✅ Logged {:.1}× {} for {}", servings, name, date);
                }
                None => println!("Product '{}' not found.", query),
            }
        }
        Commands::Daily { date, week } => {
            if week {
                let end = date.clone().unwrap_or_else(today);
                let start = date_minus_days(&end, 6);
                let range = daily_log.date_range_summary(&start, &end)?;
                display::print_weekly_summary(&start, &end, &range);
            } else {
                let date = date.unwrap_or_else(today);
                let summary = daily_log.summary(&date)?;
                display::print_daily_summary(&date, &summary);
            }
        }
        Commands::ClearDay { date } => {
            let date = date.unwrap_or_else(today);
            let removed = daily_log.clear_date(&date)?;
            println!("Cleared {} entries for {}.", removed, date);
        }
        Commands::Refresh { days, limit } => {
            let stale = db.stale_codes(days)?;
            if stale.is_empty() {
                println!("No stale entries (all updated within {} days).", days);
                return Ok(());
            }
            let to_refresh = &stale[..stale.len().min(limit as usize)];
            println!("Refreshing {} stale product(s)...", to_refresh.len());
            let mut refreshed = 0u32;
            for code in to_refresh {
                match api.get_by_barcode(code).await {
                    Ok(Some(p)) => {
                        db.upsert(&p)?;
                        refreshed += 1;
                    }
                    Ok(None) => {
                        // Product gone from API; leave cache as-is
                    }
                    Err(e) => {
                        eprintln!("  Failed to refresh {}: {}", code, e);
                    }
                }
            }
            println!("Refreshed {}/{} products.", refreshed, to_refresh.len());
        }
    }

    Ok(())
}

async fn find_product(
    db: &cache::Cache,
    api: &api::OpenFoodFactsApi,
    query: &str,
) -> Result<Option<api::Product>, Box<dyn std::error::Error>> {
    let cached = db.search(query)?;
    if let Some(p) = cached.into_iter().next() {
        return Ok(Some(p));
    }
    let results = api.search(query, 1).await?;
    if let Some(p) = results.into_iter().next() {
        db.upsert(&p)?;
        Ok(Some(p))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_cache_path_tilde() {
        std::env::set_var("HOME", "/test/home");
        let p = resolve_cache_path("~/.nutriscan/cache.db");
        assert_eq!(p, PathBuf::from("/test/home/.nutriscan/cache.db"));
    }

    #[test]
    fn test_resolve_cache_path_absolute() {
        let p = resolve_cache_path("/tmp/cache.db");
        assert_eq!(p, PathBuf::from("/tmp/cache.db"));
    }

    #[test]
    fn test_days_to_ymd_epoch() {
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_ymd_known_date() {
        // 2026-03-04 = day 20516 from epoch
        let (y, m, d) = days_to_ymd(20516);
        assert_eq!((y, m, d), (2026, 3, 4));
    }

    #[test]
    fn test_today_format() {
        let t = today();
        assert_eq!(t.len(), 10);
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[7..8], "-");
    }

    #[test]
    fn test_ymd_to_days_roundtrip() {
        let days = ymd_to_days(2026, 3, 4);
        let (y, m, d) = days_to_ymd(days);
        assert_eq!((y, m, d), (2026, 3, 4));
    }

    #[test]
    fn test_date_minus_days() {
        assert_eq!(date_minus_days("2026-03-04", 6), "2026-02-26");
        assert_eq!(date_minus_days("2026-01-03", 3), "2025-12-31");
    }

}
