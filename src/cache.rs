use crate::api::Product;
use rusqlite::{params, Connection, Result as SqlResult};
use std::path::Path;

pub struct Cache {
    conn: Connection,
}

impl Cache {
    pub fn open<P: AsRef<Path>>(path: P) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let cache = Self { conn };
        cache.init_tables()?;
        Ok(cache)
    }

    #[allow(dead_code)]
    pub fn open_in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        let cache = Self { conn };
        cache.init_tables()?;
        Ok(cache)
    }


    /// Map a database row to a Product (shared across queries).
    fn row_to_product(row: &rusqlite::Row) -> rusqlite::Result<Product> {
        let additives_json: Option<String> = row.get(5)?;
        let nutriments_json: Option<String> = row.get(6)?;
        let allergens_json: Option<String> = row.get(10)?;
        Ok(Product {
            code: row.get(0)?,
            product_name: row.get(1)?,
            brands: row.get(2)?,
            nutriscore_grade: row.get(3)?,
            nova_group: row.get(4)?,
            additives_tags: additives_json.and_then(|s| serde_json::from_str(&s).ok()),
            nutriments: nutriments_json.and_then(|s| serde_json::from_str(&s).ok()),
            ingredients_text: row.get(7)?,
            categories: row.get(8)?,
            allergens_tags: allergens_json.and_then(|s| serde_json::from_str(&s).ok()),
            image_url: row.get(9)?,
        })
    }

    fn init_tables(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS products (
                code TEXT PRIMARY KEY,
                product_name TEXT,
                brands TEXT,
                nutriscore_grade TEXT,
                nova_group INTEGER,
                additives_json TEXT,
                nutriments_json TEXT,
                ingredients_text TEXT,
                categories TEXT,
                image_url TEXT,
                allergens_json TEXT,
                updated_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_product_name ON products(product_name);",
        )
    }

    pub fn upsert(&self, p: &Product) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO products
             (code, product_name, brands, nutriscore_grade, nova_group,
              additives_json, nutriments_json, ingredients_text, categories, image_url, allergens_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                p.code,
                p.product_name,
                p.brands,
                p.nutriscore_grade,
                p.nova_group,
                serde_json::to_string(&p.additives_tags).ok(),
                serde_json::to_string(&p.nutriments).ok(),
                p.ingredients_text,
                p.categories,
                p.image_url,
                serde_json::to_string(&p.allergens_tags).ok(),
            ],
        )?;
        Ok(())
    }

    pub fn search(&self, query: &str) -> SqlResult<Vec<Product>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT code, product_name, brands, nutriscore_grade, nova_group,
                    additives_json, nutriments_json, ingredients_text, categories, image_url,
                    allergens_json
             FROM products
             WHERE product_name LIKE ?1 OR brands LIKE ?1 OR categories LIKE ?1
             LIMIT 20",
        )?;
        let rows = stmt.query_map(params![pattern], Self::row_to_product)?;
        rows.collect()
    }

    #[allow(dead_code)]
    pub fn get_by_code(&self, code: &str) -> SqlResult<Option<Product>> {
        let mut results = self.search_exact_code(code)?;
        Ok(if results.is_empty() { None } else { Some(results.remove(0)) })
    }

    #[allow(dead_code)]
    fn search_exact_code(&self, code: &str) -> SqlResult<Vec<Product>> {
        let mut stmt = self.conn.prepare(
            "SELECT code, product_name, brands, nutriscore_grade, nova_group,
                    additives_json, nutriments_json, ingredients_text, categories, image_url,
                    allergens_json
             FROM products WHERE code = ?1",
        )?;
        let rows = stmt.query_map(params![code], Self::row_to_product)?;
        rows.collect()
    }

    pub fn count(&self) -> SqlResult<i64> {
        self.conn.query_row("SELECT COUNT(*) FROM products", [], |row| row.get(0))
    }

    /// Remove products older than `days` from the cache.
    #[allow(dead_code)]
    pub fn evict_stale(&self, days: u32) -> SqlResult<usize> {
        let affected = self.conn.execute(
            "DELETE FROM products WHERE updated_at < datetime('now', ?1)",
            params![format!("-{} days", days)],
        )?;
        Ok(affected)
    }

    /// Return codes of products older than `days`.
    #[allow(dead_code)]
    pub fn stale_codes(&self, days: u32) -> SqlResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT code FROM products WHERE updated_at < datetime('now', ?1)",
        )?;
        let rows = stmt.query_map(params![format!("-{} days", days)], |row| row.get(0))?;
        rows.collect()
    }

    #[allow(dead_code)]
    pub fn clear(&self) -> SqlResult<()> {
        self.conn.execute("DELETE FROM products", [])?;
        Ok(())
    }

    /// Return all cached products ordered by most recently updated.
    pub fn recent(&self, limit: u32) -> SqlResult<Vec<Product>> {
        let mut stmt = self.conn.prepare(
            "SELECT code, product_name, brands, nutriscore_grade, nova_group,
                    additives_json, nutriments_json, ingredients_text, categories, image_url,
                    allergens_json
             FROM products ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], Self::row_to_product)?;
        rows.collect()
    }

    /// Export all cached products as a JSON string.
    pub fn export_json(&self) -> SqlResult<String> {
        let products = self.recent(u32::MAX)?;
        Ok(serde_json::to_string_pretty(&products).unwrap_or_else(|_| "[]".to_string()))
    }

    /// Return cache database size in bytes and product count.
    pub fn size_info(&self) -> SqlResult<(i64, i64)> {
        let count: i64 = self.count()?;
        let page_count: i64 = self.conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let page_size: i64 = self.conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        Ok((page_count * page_size, count))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Nutriments;

    fn sample_product(code: &str, name: &str) -> Product {
        Product {
            code: code.to_string(),
            product_name: Some(name.to_string()),
            brands: Some("TestBrand".to_string()),
            nutriscore_grade: Some("b".to_string()),
            nova_group: Some(2),
            additives_tags: Some(vec!["en:e150a".to_string()]),
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(42.0),
                fat_100g: Some(0.0),
                saturated_fat_100g: Some(0.0),
                sugars_100g: Some(10.6),
                salt_100g: Some(0.02),
                proteins_100g: Some(0.0),
                fiber_100g: None,
                carbohydrates_100g: Some(10.6),
            }),
            ingredients_text: Some("water, sugar".to_string()),
            categories: Some("beverages".to_string()),
            allergens_tags: None,
            image_url: None,
        }
    }

    #[test]
    fn test_open_in_memory() {
        let cache = Cache::open_in_memory().unwrap();
        assert_eq!(cache.count().unwrap(), 0);
    }

    #[test]
    fn test_upsert_and_count() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&sample_product("111", "Cola")).unwrap();
        assert_eq!(cache.count().unwrap(), 1);
    }

    #[test]
    fn test_search() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&sample_product("111", "Coca Cola")).unwrap();
        cache.upsert(&sample_product("222", "Pepsi")).unwrap();
        let results = cache.search("Cola").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].product_name.as_deref(), Some("Coca Cola"));
    }

    #[test]
    fn test_get_by_code() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&sample_product("5449000000996", "Coca Cola")).unwrap();
        let p = cache.get_by_code("5449000000996").unwrap().unwrap();
        assert_eq!(p.product_name.as_deref(), Some("Coca Cola"));
    }

    #[test]
    fn test_get_by_code_missing() {
        let cache = Cache::open_in_memory().unwrap();
        assert!(cache.get_by_code("9999").unwrap().is_none());
    }

    #[test]
    fn test_clear() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&sample_product("1", "A")).unwrap();
        cache.clear().unwrap();
        assert_eq!(cache.count().unwrap(), 0);
    }

    #[test]
    fn test_search_by_brand() {
        let cache = Cache::open_in_memory().unwrap();
        let mut p = sample_product("333", "Mystery Drink");
        p.brands = Some("SpecialBrand".to_string());
        cache.upsert(&p).unwrap();
        let results = cache.search("SpecialBrand").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_by_category() {
        let cache = Cache::open_in_memory().unwrap();
        let mut p = sample_product("444", "Juice");
        p.categories = Some("fruit juices".to_string());
        cache.upsert(&p).unwrap();
        let results = cache.search("fruit").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_evict_stale_no_stale() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&sample_product("1", "Fresh")).unwrap();
        let evicted = cache.evict_stale(30).unwrap();
        assert_eq!(evicted, 0);
        assert_eq!(cache.count().unwrap(), 1);
    }

    #[test]
    fn test_stale_codes_empty() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&sample_product("1", "Fresh")).unwrap();
        let stale = cache.stale_codes(30).unwrap();
        assert!(stale.is_empty());
    }

    #[test]
    fn test_upsert_overwrites() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&sample_product("1", "Old")).unwrap();
        cache.upsert(&sample_product("1", "New")).unwrap();
        assert_eq!(cache.count().unwrap(), 1);
        let p = cache.get_by_code("1").unwrap().unwrap();
        assert_eq!(p.product_name.as_deref(), Some("New"));
    }

    #[test]
    fn test_recent_ordering() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&sample_product("1", "First")).unwrap();
        cache.upsert(&sample_product("2", "Second")).unwrap();
        let recent = cache.recent(10).unwrap();
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_recent_limit() {
        let cache = Cache::open_in_memory().unwrap();
        for i in 0..5 {
            cache.upsert(&sample_product(&i.to_string(), &format!("P{}", i))).unwrap();
        }
        let recent = cache.recent(3).unwrap();
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_export_json() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&sample_product("1", "Export Test")).unwrap();
        let json = cache.export_json().unwrap();
        assert!(json.contains("Export Test"));
        // Verify it's valid JSON
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn test_export_json_empty() {
        let cache = Cache::open_in_memory().unwrap();
        let json = cache.export_json().unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_size_info() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&sample_product("1", "Size")).unwrap();
        let (bytes, count) = cache.size_info().unwrap();
        assert!(bytes > 0);
        assert_eq!(count, 1);
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use crate::api::Nutriments;

    fn sample_product(code: &str, name: &str) -> Product {
        Product {
            code: code.to_string(),
            product_name: Some(name.to_string()),
            brands: Some("TestBrand".to_string()),
            nutriscore_grade: Some("b".to_string()),
            nova_group: Some(2),
            additives_tags: None,
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(100.0),
                ..Default::default()
            }),
            ingredients_text: None,
            categories: None,
            allergens_tags: None,
            image_url: None,
        }
    }

    #[test]
    fn test_export_import_roundtrip() {
        // Export from one cache, import into another
        let src = Cache::open_in_memory().unwrap();
        src.upsert(&sample_product("001", "Alpha")).unwrap();
        src.upsert(&sample_product("002", "Beta")).unwrap();
        let json = src.export_json().unwrap();

        let products: Vec<Product> = serde_json::from_str(&json).unwrap();
        assert_eq!(products.len(), 2);

        let dst = Cache::open_in_memory().unwrap();
        for p in &products {
            dst.upsert(p).unwrap();
        }
        assert_eq!(dst.count().unwrap(), 2);
        assert!(dst.get_by_code("001").unwrap().is_some());
        assert!(dst.get_by_code("002").unwrap().is_some());
    }
}

impl Cache {
    /// Import multiple products in a single transaction for much better performance.
    /// Returns (new_count, updated_count).
    pub fn import_products(&self, products: &[Product]) -> SqlResult<(usize, usize)> {
        let tx = self.conn.unchecked_transaction()?;
        let mut new_count = 0usize;
        let mut updated_count = 0usize;
        for p in products {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE code = ?1)",
                params![p.code],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO products
                 (code, product_name, brands, nutriscore_grade, nova_group,
                  additives_json, nutriments_json, ingredients_text, categories, image_url, allergens_json)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    p.code,
                    p.product_name,
                    p.brands,
                    p.nutriscore_grade,
                    p.nova_group,
                    serde_json::to_string(&p.additives_tags).ok(),
                    serde_json::to_string(&p.nutriments).ok(),
                    p.ingredients_text,
                    p.categories,
                    p.image_url,
                    serde_json::to_string(&p.allergens_tags).ok(),
                ],
            )?;
            if exists {
                updated_count += 1;
            } else {
                new_count += 1;
            }
        }
        tx.commit()?;
        Ok((new_count, updated_count))
    }
}

#[cfg(test)]
mod import_bulk_tests {
    use super::*;
    use crate::api::Nutriments;

    fn sample(code: &str, name: &str) -> Product {
        Product {
            code: code.to_string(),
            product_name: Some(name.to_string()),
            brands: None,
            nutriscore_grade: None,
            nova_group: None,
            additives_tags: None,
            nutriments: Some(Nutriments { energy_kcal_100g: Some(100.0), ..Default::default() }),
            ingredients_text: None,
            categories: None,
            allergens_tags: None,
            image_url: None,
        }
    }

    #[test]
    fn test_import_products_new() {
        let cache = Cache::open_in_memory().unwrap();
        let products = vec![sample("1", "A"), sample("2", "B")];
        let (new_c, upd_c) = cache.import_products(&products).unwrap();
        assert_eq!(new_c, 2);
        assert_eq!(upd_c, 0);
        assert_eq!(cache.count().unwrap(), 2);
    }

    #[test]
    fn test_import_products_mixed() {
        let cache = Cache::open_in_memory().unwrap();
        cache.upsert(&sample("1", "Old")).unwrap();
        let products = vec![sample("1", "Updated"), sample("2", "New")];
        let (new_c, upd_c) = cache.import_products(&products).unwrap();
        assert_eq!(new_c, 1);
        assert_eq!(upd_c, 1);
        assert_eq!(cache.count().unwrap(), 2);
        let p = cache.get_by_code("1").unwrap().unwrap();
        assert_eq!(p.product_name.as_deref(), Some("Updated"));
    }

    #[test]
    fn test_import_products_empty() {
        let cache = Cache::open_in_memory().unwrap();
        let (new_c, upd_c) = cache.import_products(&[]).unwrap();
        assert_eq!(new_c, 0);
        assert_eq!(upd_c, 0);
    }
}
