mod api;
mod analyzer;
mod cache;
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
    /// Show cache statistics
    Stats,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let cache_path = resolve_cache_path(&cli.cache_dir);

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = cache::Cache::open(&cache_path)?;
    let api = api::OpenFoodFactsApi::new();

    match cli.command {
        Commands::Scan { query } => {
            // Try cache first
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
        Commands::Stats => {
            let count = db.count()?;
            println!("Cache: {} products stored.", count);
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
