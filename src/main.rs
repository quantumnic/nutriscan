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
    /// Import products from a JSON file (exported via export)
    Import {
        /// Input file path
        #[arg(short, long, default_value = "nutriscan-export.json")]
        input: String,
    },
    /// Purge stale cache entries older than N days
    Purge {
        /// Maximum age in days
        #[arg(short, long, default_value_t = 90)]
        days: u32,
    },
    /// Log a product to daily intake tracker
    Log {
        /// Date in YYYY-MM-DD format (defaults to today)
        #[arg(short, long)]
        date: Option<String>,
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

    /// Undo the last logged entry for today (or a specific date)
    Undo {
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

/// Validate and normalize a YYYY-MM-DD date string.
/// Returns Ok(date) if valid, Err with message otherwise.
fn validate_date(s: &str) -> Result<String, String> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return Err(format!("Invalid date format '{}': expected YYYY-MM-DD", s));
    }
    let y: u64 = parts[0].parse().map_err(|_| format!("Invalid year in '{}'", s))?;
    let m: u64 = parts[1].parse().map_err(|_| format!("Invalid month in '{}'", s))?;
    let d: u64 = parts[2].parse().map_err(|_| format!("Invalid day in '{}'", s))?;
    if !(1..=12).contains(&m) {
        return Err(format!("Month {} out of range (1-12) in '{}'", m, s));
    }
    let max_day = days_in_month(y, m);
    if !(1..=max_day).contains(&d) {
        return Err(format!("Day {} out of range (1-{}) for {}-{:02}", d, max_day, y, m));
    }
    Ok(format!("{:04}-{:02}-{:02}", y, m, d))
}

/// Return the number of days in a given month.
fn days_in_month(y: u64, m: u64) -> u64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)) { 29 } else { 28 },
        _ => 30,
    }
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
            // Daily log stats
            let daily_stats = daily_log.stats()?;

            let (bytes, count) = db.size_info()?;
            let kb = bytes as f64 / 1024.0;
            println!("Cache: {} products stored ({:.1} KB)", count, kb);
            if daily_stats.logged_days > 0 {
                println!(
                    "Daily log: {} entries across {} day(s) ({} → {})",
                    daily_stats.total_entries,
                    daily_stats.logged_days,
                    daily_stats.first_date.as_deref().unwrap_or("?"),
                    daily_stats.last_date.as_deref().unwrap_or("?"),
                );
                let streak = daily_log.streak(&today())?;
                if streak > 0 {
                    println!("Streak: {} consecutive day(s)", streak);
                }
                let top = daily_log.top_products(5)?;
                if !top.is_empty() {
                    println!("Top products:");
                    for (i, tp) in top.iter().enumerate() {
                        println!(
                            "  {}. {} — logged {} time{} ({:.1} total servings)",
                            i + 1,
                            tp.product_name,
                            tp.times_logged,
                            if tp.times_logged == 1 { "" } else { "s" },
                            tp.total_servings,
                        );
                    }
                }
            } else {
                println!("Daily log: no entries yet.");
            }
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
        Commands::Import { input } => {
            let data = std::fs::read_to_string(&input)?;
            let products: Vec<crate::api::Product> = serde_json::from_str(&data)?;
            let count = products.len();
            for p in &products {
                db.upsert(p)?;
            }
            println!("Imported {} products from {}", count, input);
        }
        Commands::Purge { days } => {
            let evicted = db.evict_stale(days)?;
            println!("Purged {} stale entries (older than {} days).", evicted, days);
        }
        Commands::Log { query, servings, date } => {
            let product = find_product(&db, &api, &query).await?;
            match product {
                Some(p) => {
                    let date = match date {
                        Some(d) => validate_date(&d)?,
                        None => today(),
                    };
                    let name = p.product_name.clone().unwrap_or_else(|| "Unknown".into());
                    daily_log.log_product(&date, &p, servings)?;
                    db.upsert(&p)?;
                    println!("✅ Logged {:.1}× {} for {}", servings, name, date);

                    // Show running daily totals after logging
                    let summary = daily_log.summary(&date)?;
                    let entry_count = summary.entries.len();
                    let day_label = if date == today() { "Today so far".to_string() } else { format!("{} so far", date) };
                    println!(
                        "   📊 {}: {:.0} kcal | P {:.0}g | C {:.0}g | F {:.0}g ({} item{})",
                        day_label,
                        summary.total_kcal,
                        summary.total_protein,
                        summary.total_carbs,
                        summary.total_fat,
                        entry_count,
                        if entry_count == 1 { "" } else { "s" },
                    );
                    let remaining = 2000.0 - summary.total_kcal;
                    if remaining > 0.0 {
                        println!("   🎯 {:.0} kcal remaining (of 2000 kcal daily target)", remaining);
                    } else {
                        println!("   ⚡ Daily target of 2000 kcal reached!");
                    }
                }
                None => println!("Product '{}' not found.", query),
            }
        }
        Commands::Daily { date, week } => {
            if week {
                let end = match date {
                    Some(d) => validate_date(&d)?,
                    None => today(),
                };
                let start = date_minus_days(&end, 6);
                let range = daily_log.date_range_summary(&start, &end)?;
                let streak = daily_log.streak(&end)?;
                display::print_weekly_summary(&start, &end, &range, streak);
            } else {
                let date = match date {
                    Some(d) => validate_date(&d)?,
                    None => today(),
                };
                let summary = daily_log.summary(&date)?;
                let streak = daily_log.streak(&date)?;
                display::print_daily_summary(&date, &summary, streak);
            }
        }
        Commands::ClearDay { date } => {
            let date = match date {
                Some(d) => validate_date(&d)?,
                None => today(),
            };
            let removed = daily_log.clear_date(&date)?;
            println!("Cleared {} entries for {}.", removed, date);
        }
        Commands::Undo { date } => {
            let date = match date {
                Some(d) => validate_date(&d)?,
                None => today(),
            };
            match daily_log.undo_last(&date)? {
                Some(name) => {
                    println!("↩ Removed last entry: {} ({})", name, date);
                    // Show updated daily totals after undo
                    let summary = daily_log.summary(&date)?;
                    let entry_count = summary.entries.len();
                    if entry_count > 0 {
                        let day_label = if date == today() { "Today now".to_string() } else { format!("{} now", date) };
                        println!(
                            "   📊 {}: {:.0} kcal | P {:.0}g | C {:.0}g | F {:.0}g ({} item{})",
                            day_label,
                            summary.total_kcal,
                            summary.total_protein,
                            summary.total_carbs,
                            summary.total_fat,
                            entry_count,
                            if entry_count == 1 { "" } else { "s" },
                        );
                        let remaining = 2000.0 - summary.total_kcal;
                        if remaining > 0.0 {
                            println!("   🎯 {:.0} kcal remaining (of 2000 kcal daily target)", remaining);
                        } else {
                            println!("   ⚡ Daily target of 2000 kcal reached!");
                        }
                    } else {
                        println!("   📊 Day is now empty.");
                    }
                }
                None => println!("No entries to undo for {}.", date),
            }
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

    #[test]
    fn test_validate_date_valid() {
        assert_eq!(validate_date("2026-03-14").unwrap(), "2026-03-14");
        assert_eq!(validate_date("2024-02-29").unwrap(), "2024-02-29"); // leap year
        assert_eq!(validate_date("2026-1-5").unwrap(), "2026-01-05"); // normalizes
    }

    #[test]
    fn test_validate_date_invalid_month() {
        assert!(validate_date("2026-13-01").is_err());
        assert!(validate_date("2026-00-01").is_err());
    }

    #[test]
    fn test_validate_date_invalid_day() {
        assert!(validate_date("2026-02-29").is_err()); // not a leap year
        assert!(validate_date("2026-04-31").is_err()); // April has 30 days
    }

    #[test]
    fn test_validate_date_bad_format() {
        assert!(validate_date("20260314").is_err());
        assert!(validate_date("not-a-date").is_err());
        assert!(validate_date("2026-03").is_err());
    }

    #[test]
    fn test_days_in_month() {
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29); // leap year
        assert_eq!(days_in_month(2000, 2), 29); // century leap
        assert_eq!(days_in_month(1900, 2), 28); // century non-leap
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 4), 30);
    }

}
