# NutriScan 🥗

Offline food analysis using [Open Food Facts](https://world.openfoodfacts.org/) data — Nutri-Score, additives, NOVA group. CLI tool written in Rust.

## Features

- **Scan** products by name or barcode → nutrition info, Nutri-Score (A–E), NOVA group (1–4)
- **Warn** about problematic additives in products
- **Compare** two products side-by-side
- **Offline-first** — local SQLite cache, fetches from API only when needed
- **Update** cache with bulk downloads

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Scan a product
nutriscan scan "Coca Cola"

# Check additive warnings
nutriscan warn "Coca Cola"

# Compare two products
nutriscan compare "Coca Cola" "Pepsi"

# Update local cache
nutriscan update "beverages" --limit 100

# Cache statistics
nutriscan stats
```

## How it works

1. Searches the local SQLite cache first (offline-first)
2. Falls back to the Open Food Facts API if not cached
3. Automatically caches API results for future offline use
4. Analyzes Nutri-Score, NOVA classification, and known problematic additives

## Tech Stack

- **Rust** with async runtime (tokio)
- **reqwest** for HTTP
- **rusqlite** for local SQLite cache
- **serde** for JSON serialization
- **clap** for CLI argument parsing
- **colored** for terminal output

## License

MIT
