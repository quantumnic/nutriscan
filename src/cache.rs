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
                updated_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_product_name ON products(product_name);",
        )
    }

    pub fn upsert(&self, p: &Product) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO products
             (code, product_name, brands, nutriscore_grade, nova_group,
              additives_json, nutriments_json, ingredients_text, categories, image_url)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
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
            ],
        )?;
        Ok(())
    }

    pub fn search(&self, query: &str) -> SqlResult<Vec<Product>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT code, product_name, brands, nutriscore_grade, nova_group,
                    additives_json, nutriments_json, ingredients_text, categories, image_url
             FROM products
             WHERE product_name LIKE ?1 OR brands LIKE ?1 OR categories LIKE ?1
             LIMIT 20",
        )?;
        let rows = stmt.query_map(params![pattern], |row| {
            let additives_json: Option<String> = row.get(5)?;
            let nutriments_json: Option<String> = row.get(6)?;
            Ok(Product {
                code: row.get(0)?,
                product_name: row.get(1)?,
                brands: row.get(2)?,
                nutriscore_grade: row.get(3)?,
                nova_group: row.get(4)?,
                additives_tags: additives_json
                    .and_then(|s| serde_json::from_str(&s).ok()),
                nutriments: nutriments_json
                    .and_then(|s| serde_json::from_str(&s).ok()),
                ingredients_text: row.get(7)?,
                categories: row.get(8)?,
                image_url: row.get(9)?,
            })
        })?;
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
                    additives_json, nutriments_json, ingredients_text, categories, image_url
             FROM products WHERE code = ?1",
        )?;
        let rows = stmt.query_map(params![code], |row| {
            let additives_json: Option<String> = row.get(5)?;
            let nutriments_json: Option<String> = row.get(6)?;
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
                image_url: row.get(9)?,
            })
        })?;
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
}
