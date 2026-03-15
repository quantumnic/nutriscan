use crate::api::{Nutriments, Product};
use rusqlite::{params, Connection, Result as SqlResult};
use std::path::Path;

/// Tracks daily food intake with per-serving logging.
pub struct DailyLog {
    conn: Connection,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DailyEntry {
    pub code: String,
    pub product_name: String,
    pub servings: f64,
    pub logged_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct DailySummary {
    pub entries: Vec<DailyEntry>,
    pub total_kcal: f64,
    pub total_fat: f64,
    pub total_carbs: f64,
    pub total_protein: f64,
    pub total_sugar: f64,
    pub total_salt: f64,
    pub total_fiber: f64,
    pub total_saturated_fat: f64,
    /// Highest-calorie entry: (product_name, kcal contributed)
    pub top_kcal_entry: Option<(String, f64)>,
}

impl DailySummary {
    /// Quick textual verdict on the day's intake.
    pub fn verdict(&self) -> String {
        if self.entries.is_empty() {
            return "No entries logged today.".to_string();
        }
        let mut warnings: Vec<&str> = Vec::new();

        let cal_msg = match self.total_kcal as u32 {
            0..=1200 => "Low calorie intake — make sure you're eating enough!",
            1201..=2200 => "Calorie intake looks reasonable.",
            2201..=2800 => "Slightly above average — fine if you're active.",
            _ => "High calorie intake — consider lighter options.",
        };

        if self.total_salt > 5.0 {
            warnings.push("⚠ Salt exceeds 5 g (WHO daily limit).");
        }
        if self.total_sugar > 50.0 {
            warnings.push("⚠ Sugar exceeds 50 g (WHO daily limit).");
        }
        if self.total_saturated_fat > 20.0 {
            warnings.push("⚠ Saturated fat exceeds 20 g (recommended limit).");
        }
        if self.total_fiber < 25.0 && self.total_kcal > 500.0 {
            warnings.push("💡 Fiber below 25 g (WHO daily recommendation).");
        }
        if self.total_protein < 50.0 && self.total_kcal > 500.0 {
            warnings.push("💡 Protein below 50 g (recommended daily minimum).");
        }
        if warnings.is_empty() {
            cal_msg.to_string()
        } else {
            format!("{} {}", cal_msg, warnings.join(" "))
        }
    }

    /// Caloric macro breakdown as percentages (fat%, carbs%, protein%).
    /// Fat = 9 kcal/g, carbs & protein = 4 kcal/g each.
    pub fn macro_percentages(&self) -> Option<(f64, f64, f64)> {
        let fat_cal = self.total_fat * 9.0;
        let carb_cal = self.total_carbs * 4.0;
        let prot_cal = self.total_protein * 4.0;
        let total = fat_cal + carb_cal + prot_cal;
        if total < 1.0 {
            return None;
        }
        Some((
            fat_cal / total * 100.0,
            carb_cal / total * 100.0,
            prot_cal / total * 100.0,
        ))
    }
}

impl DailyLog {
    pub fn open<P: AsRef<Path>>(path: P) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let log = Self { conn };
        log.init_tables()?;
        Ok(log)
    }

    #[allow(dead_code)]
    pub fn open_in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        let log = Self { conn };
        log.init_tables()?;
        Ok(log)
    }

    fn init_tables(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS daily_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL,
                code TEXT NOT NULL,
                product_name TEXT NOT NULL,
                servings REAL NOT NULL DEFAULT 1.0,
                nutriments_json TEXT,
                logged_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_daily_date ON daily_log(date);",
        )
    }

    /// Log a product with a given number of servings for today.
    pub fn log_product(&self, date: &str, product: &Product, servings: f64) -> SqlResult<()> {
        let name = product.display_name();
        let nutriments_json = serde_json::to_string(&product.nutriments).ok();
        self.conn.execute(
            "INSERT INTO daily_log (date, code, product_name, servings, nutriments_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![date, product.code, name, servings, nutriments_json],
        )?;
        Ok(())
    }

    /// Get summary for a given date.
    pub fn summary(&self, date: &str) -> SqlResult<DailySummary> {
        let mut stmt = self.conn.prepare(
            "SELECT code, product_name, servings, nutriments_json, logged_at
             FROM daily_log WHERE date = ?1 ORDER BY logged_at",
        )?;
        let mut summary = DailySummary::default();

        let rows = stmt.query_map(params![date], |row| {
            let code: String = row.get(0)?;
            let product_name: String = row.get(1)?;
            let servings: f64 = row.get(2)?;
            let nutriments_json: Option<String> = row.get(3)?;
            let logged_at: String = row.get(4)?;
            let nutriments: Option<Nutriments> =
                nutriments_json.and_then(|s| serde_json::from_str(&s).ok());
            Ok((
                DailyEntry { code, product_name, servings, logged_at },
                nutriments,
                servings,
            ))
        })?;

        for row in rows {
            let (entry, nutriments, servings) = row?;
            if let Some(n) = nutriments {
                summary.total_kcal += n.energy_kcal_or_estimated().unwrap_or(0.0) * servings;
                summary.total_fat += n.fat_100g.unwrap_or(0.0) * servings;
                summary.total_carbs += n.carbohydrates_100g.unwrap_or(0.0) * servings;
                summary.total_protein += n.proteins_100g.unwrap_or(0.0) * servings;
                summary.total_sugar += n.sugars_100g.unwrap_or(0.0) * servings;
                summary.total_salt += n.salt_100g.unwrap_or(0.0) * servings;
                summary.total_fiber += n.fiber_100g.unwrap_or(0.0) * servings;
                summary.total_saturated_fat += n.saturated_fat_100g.unwrap_or(0.0) * servings;
                let entry_kcal = n.energy_kcal_or_estimated().unwrap_or(0.0) * servings;
                match &summary.top_kcal_entry {
                    Some((_, best)) if entry_kcal <= *best => {}
                    _ => { summary.top_kcal_entry = Some((entry.product_name.clone(), entry_kcal)); }
                }
            }
            summary.entries.push(entry);
        }

        Ok(summary)
    }

    /// Remove all entries for a date.
    pub fn clear_date(&self, date: &str) -> SqlResult<usize> {
        let affected = self.conn.execute(
            "DELETE FROM daily_log WHERE date = ?1",
            params![date],
        )?;
        Ok(affected)
    }

    /// Count entries for a date.
    #[allow(dead_code)]
    pub fn count(&self, date: &str) -> SqlResult<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM daily_log WHERE date = ?1",
            params![date],
            |row| row.get(0),
        )
    }

    /// Get summaries for a date range (inclusive).
    pub fn date_range_summary(&self, from: &str, to: &str) -> SqlResult<Vec<(String, DailySummary)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT date FROM daily_log WHERE date >= ?1 AND date <= ?2 ORDER BY date",
        )?;
        let dates: Vec<String> = stmt
            .query_map(params![from, to], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        let mut results = Vec::new();
        for date in dates {
            let summary = self.summary(&date)?;
            results.push((date, summary));
        }
        Ok(results)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Nutriments, Product};

    fn sample_product(code: &str, name: &str, kcal: f64) -> Product {
        Product {
            code: code.to_string(),
            product_name: Some(name.to_string()),
            brands: Some("Brand".to_string()),
            nutriscore_grade: Some("b".to_string()),
            nova_group: Some(2),
            additives_tags: None,
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(kcal),
                fat_100g: Some(5.0),
                saturated_fat_100g: Some(1.0),
                sugars_100g: Some(8.0),
                salt_100g: Some(0.5),
                proteins_100g: Some(3.0),
                fiber_100g: Some(2.0),
                carbohydrates_100g: Some(20.0),
            }),
            ingredients_text: None,
            categories: None,
            allergens_tags: None, image_url: None, quantity: None, serving_size: None,
        }
    }

    #[test]
    fn test_log_and_count() {
        let log = DailyLog::open_in_memory().unwrap();
        let p = sample_product("1", "Apple", 52.0);
        log.log_product("2026-03-04", &p, 1.0).unwrap();
        assert_eq!(log.count("2026-03-04").unwrap(), 1);
        assert_eq!(log.count("2026-03-05").unwrap(), 0);
    }

    #[test]
    fn test_summary_totals() {
        let log = DailyLog::open_in_memory().unwrap();
        let p = sample_product("1", "Oats", 100.0);
        log.log_product("2026-03-04", &p, 2.0).unwrap();
        let s = log.summary("2026-03-04").unwrap();
        assert_eq!(s.entries.len(), 1);
        assert!((s.total_kcal - 200.0).abs() < 0.01);
        assert!((s.total_fat - 10.0).abs() < 0.01);
        assert!((s.total_protein - 6.0).abs() < 0.01);
    }

    #[test]
    fn test_summary_multiple_products() {
        let log = DailyLog::open_in_memory().unwrap();
        log.log_product("2026-03-04", &sample_product("1", "A", 100.0), 1.0).unwrap();
        log.log_product("2026-03-04", &sample_product("2", "B", 200.0), 1.5).unwrap();
        let s = log.summary("2026-03-04").unwrap();
        assert_eq!(s.entries.len(), 2);
        assert!((s.total_kcal - (100.0 + 300.0)).abs() < 0.01);
    }

    #[test]
    fn test_summary_empty_day() {
        let log = DailyLog::open_in_memory().unwrap();
        let s = log.summary("2026-03-04").unwrap();
        assert!(s.entries.is_empty());
        assert_eq!(s.verdict(), "No entries logged today.");
    }

    #[test]
    fn test_clear_date() {
        let log = DailyLog::open_in_memory().unwrap();
        let p = sample_product("1", "A", 100.0);
        log.log_product("2026-03-04", &p, 1.0).unwrap();
        log.log_product("2026-03-04", &p, 1.0).unwrap();
        log.log_product("2026-03-05", &p, 1.0).unwrap();
        let removed = log.clear_date("2026-03-04").unwrap();
        assert_eq!(removed, 2);
        assert_eq!(log.count("2026-03-04").unwrap(), 0);
        assert_eq!(log.count("2026-03-05").unwrap(), 1);
    }

    #[test]
    fn test_verdict_low_cal() {
        let log = DailyLog::open_in_memory().unwrap();
        let p = sample_product("1", "Salad", 20.0);
        log.log_product("2026-03-04", &p, 1.0).unwrap();
        let s = log.summary("2026-03-04").unwrap();
        assert!(s.verdict().contains("Low calorie"));
    }

    #[test]
    fn test_verdict_high_cal() {
        let log = DailyLog::open_in_memory().unwrap();
        let p = sample_product("1", "Pizza", 300.0);
        log.log_product("2026-03-04", &p, 10.0).unwrap();
        let s = log.summary("2026-03-04").unwrap();
        assert!(s.verdict().contains("High calorie"));
    }

    #[test]
    fn test_log_product_no_nutriments() {
        let log = DailyLog::open_in_memory().unwrap();
        let p = Product {
            code: "x".to_string(),
            product_name: Some("Mystery".to_string()),
            brands: None, nutriscore_grade: None, nova_group: None,
            additives_tags: None, nutriments: None, ingredients_text: None,
            categories: None, allergens_tags: None, image_url: None, quantity: None, serving_size: None,
        };
        log.log_product("2026-03-04", &p, 1.0).unwrap();
        let s = log.summary("2026-03-04").unwrap();
        assert_eq!(s.entries.len(), 1);
        assert!(s.total_kcal.abs() < 0.01);
    }

    #[test]
    fn test_date_range_summary() {
        let log = DailyLog::open_in_memory().unwrap();
        let p = sample_product("1", "Oats", 100.0);
        log.log_product("2026-03-01", &p, 1.0).unwrap();
        log.log_product("2026-03-03", &p, 2.0).unwrap();
        log.log_product("2026-03-05", &p, 1.0).unwrap();
        let range = log.date_range_summary("2026-03-01", "2026-03-04").unwrap();
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].0, "2026-03-01");
        assert_eq!(range[1].0, "2026-03-03");
        assert!((range[1].1.total_kcal - 200.0).abs() < 0.01);
    }

}

#[cfg(test)]
mod sat_fat_tests {
    use super::*;
    use crate::api::{Nutriments, Product};

    #[test]
    fn test_summary_tracks_saturated_fat() {
        let log = DailyLog::open_in_memory().unwrap();
        let p = Product {
            code: "1".to_string(),
            product_name: Some("Cheese".to_string()),
            brands: Some("Brand".to_string()),
            nutriscore_grade: Some("d".to_string()),
            nova_group: Some(3),
            additives_tags: None,
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(350.0),
                fat_100g: Some(28.0),
                saturated_fat_100g: Some(18.0),
                sugars_100g: Some(0.5),
                salt_100g: Some(1.8),
                proteins_100g: Some(25.0),
                fiber_100g: Some(0.0),
                carbohydrates_100g: Some(1.3),
            }),
            ingredients_text: None,
            categories: None,
            allergens_tags: None, image_url: None, quantity: None, serving_size: None,
        };
        log.log_product("2026-03-04", &p, 2.0).unwrap();
        let s = log.summary("2026-03-04").unwrap();
        assert!((s.total_saturated_fat - 36.0).abs() < 0.01);
    }
}

#[cfg(test)]
mod verdict_warning_tests {
    use super::*;

    #[test]
    fn test_verdict_high_salt_warning() {
        let s = DailySummary {
            entries: vec![DailyEntry {
                code: "000".to_string(), product_name: "Salty Chips".to_string(), logged_at: "12:00".to_string(),
                servings: 1.0,
            }],
            total_kcal: 1500.0,
            total_fat: 10.0,
            total_carbs: 50.0,
            total_protein: 20.0,
            total_sugar: 10.0,
            total_salt: 6.5,
            total_fiber: 3.0,
            total_saturated_fat: 5.0, top_kcal_entry: None,
        };
        let v = s.verdict();
        assert!(v.contains("Salt exceeds 5 g"), "got: {v}");
        assert!(!v.contains("Sugar exceeds"), "got: {v}");
    }

    #[test]
    fn test_verdict_high_sugar_warning() {
        let s = DailySummary {
            entries: vec![DailyEntry {
                code: "000".to_string(), product_name: "Candy".to_string(), logged_at: "12:00".to_string(),
                servings: 1.0,
            }],
            total_kcal: 1800.0,
            total_fat: 5.0,
            total_carbs: 80.0,
            total_protein: 5.0,
            total_sugar: 65.0,
            total_salt: 1.0,
            total_fiber: 1.0,
            total_saturated_fat: 2.0, top_kcal_entry: None,
        };
        let v = s.verdict();
        assert!(v.contains("Sugar exceeds 50 g"), "got: {v}");
    }

    #[test]
    fn test_verdict_multiple_warnings() {
        let s = DailySummary {
            entries: vec![DailyEntry {
                code: "000".to_string(), product_name: "Junk".to_string(), logged_at: "12:00".to_string(),
                servings: 1.0,
            }],
            total_kcal: 3000.0,
            total_fat: 50.0,
            total_carbs: 100.0,
            total_protein: 30.0,
            total_sugar: 80.0,
            total_salt: 7.0,
            total_fiber: 2.0,
            total_saturated_fat: 25.0, top_kcal_entry: None,
        };
        let v = s.verdict();
        assert!(v.contains("Salt exceeds"), "got: {v}");
        assert!(v.contains("Sugar exceeds"), "got: {v}");
        assert!(v.contains("Saturated fat exceeds"), "got: {v}");
        assert!(v.contains("High calorie"), "got: {v}");
    }
}

#[cfg(test)]
mod fiber_warning_tests {
    use super::*;

    #[test]
    fn test_verdict_low_fiber_warning() {
        let s = DailySummary {
            entries: vec![DailyEntry {
                code: "000".to_string(), product_name: "White Bread".to_string(),
                logged_at: "12:00".to_string(), servings: 1.0,
            }],
            total_kcal: 1800.0,
            total_fat: 10.0,
            total_carbs: 60.0,
            total_protein: 20.0,
            total_sugar: 10.0,
            total_salt: 2.0,
            total_fiber: 8.0,
            total_saturated_fat: 5.0, top_kcal_entry: None,
        };
        let v = s.verdict();
        assert!(v.contains("Fiber below 25 g"), "got: {v}");
    }

    #[test]
    fn test_verdict_adequate_fiber_no_warning() {
        let s = DailySummary {
            entries: vec![DailyEntry {
                code: "000".to_string(), product_name: "Lentils".to_string(),
                logged_at: "12:00".to_string(), servings: 1.0,
            }],
            total_kcal: 1800.0,
            total_fat: 10.0,
            total_carbs: 60.0,
            total_protein: 30.0,
            total_sugar: 10.0,
            total_salt: 2.0,
            total_fiber: 30.0,
            total_saturated_fat: 3.0, top_kcal_entry: None,
        };
        let v = s.verdict();
        assert!(!v.contains("Fiber below"), "got: {v}");
    }

    #[test]
    fn test_verdict_low_fiber_skipped_if_low_intake() {
        let s = DailySummary {
            entries: vec![DailyEntry {
                code: "000".to_string(), product_name: "Snack".to_string(),
                logged_at: "12:00".to_string(), servings: 1.0,
            }],
            total_kcal: 200.0,
            total_fat: 2.0,
            total_carbs: 10.0,
            total_protein: 5.0,
            total_sugar: 3.0,
            total_salt: 0.5,
            total_fiber: 2.0,
            total_saturated_fat: 1.0, top_kcal_entry: None,
        };
        let v = s.verdict();
        assert!(!v.contains("Fiber below"), "shouldn't warn on low intake, got: {v}");
    }

    #[test]
    fn test_verdict_low_protein() {
        let summary = DailySummary {
            entries: vec![DailyEntry {
                code: "1".into(),
                product_name: "Test".into(),
                servings: 1.0,
                logged_at: "2025-01-01 12:00".into(),
            }],
            total_kcal: 1800.0,
            total_fat: 60.0,
            total_carbs: 200.0,
            total_protein: 30.0,
            total_sugar: 20.0,
            total_salt: 2.0,
            total_fiber: 30.0,
            total_saturated_fat: 10.0, top_kcal_entry: None,
        };
        let v = summary.verdict();
        assert!(v.contains("Protein below 50 g"), "got: {v}");
    }
}

#[cfg(test)]
mod macro_pct_tests {
    use super::*;

    #[test]
    fn test_macro_percentages_balanced() {
        let s = DailySummary {
            entries: vec![DailyEntry {
                code: "1".into(), product_name: "Mix".into(),
                servings: 1.0, logged_at: "12:00".into(),
            }],
            total_kcal: 2000.0,
            total_fat: 67.0,   // 603 kcal
            total_carbs: 250.0, // 1000 kcal
            total_protein: 100.0, // 400 kcal  => total ~2003
            total_sugar: 30.0, total_salt: 3.0,
            total_fiber: 25.0, total_saturated_fat: 10.0, top_kcal_entry: None,
        };
        let (f, c, p) = s.macro_percentages().unwrap();
        assert!((f - 30.1).abs() < 1.0, "fat%: {f}");
        assert!((c - 49.9).abs() < 1.0, "carb%: {c}");
        assert!((p - 20.0).abs() < 1.0, "protein%: {p}");
    }

    #[test]
    fn test_macro_percentages_empty() {
        let s = DailySummary::default();
        assert!(s.macro_percentages().is_none());
    }
}


impl DailyLog {
    /// Remove the most recently logged entry for a date. Returns the product name if removed.
    pub fn undo_last(&self, date: &str) -> SqlResult<Option<String>> {
        let row: Option<(i64, String)> = self.conn.query_row(
            "SELECT id, product_name FROM daily_log WHERE date = ?1 ORDER BY id DESC LIMIT 1",
            params![date],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).ok();
        match row {
            Some((id, name)) => {
                self.conn.execute("DELETE FROM daily_log WHERE id = ?1", params![id])?;
                Ok(Some(name))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod undo_tests {
    use super::*;
    use crate::api::{Nutriments, Product};

    fn sample(name: &str, kcal: f64) -> Product {
        Product {
            code: "1".to_string(),
            product_name: Some(name.to_string()),
            brands: Some("B".to_string()),
            nutriscore_grade: None, nova_group: None,
            additives_tags: None,
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(kcal), fat_100g: Some(5.0),
                saturated_fat_100g: Some(1.0), sugars_100g: Some(8.0),
                salt_100g: Some(0.5), proteins_100g: Some(3.0),
                fiber_100g: Some(2.0), carbohydrates_100g: Some(20.0),
            }),
            ingredients_text: None, categories: None, allergens_tags: None, image_url: None, quantity: None, serving_size: None,
        }
    }

    #[test]
    fn test_undo_last_removes_most_recent() {
        let log = DailyLog::open_in_memory().unwrap();
        log.log_product("2026-03-05", &sample("Apple", 52.0), 1.0).unwrap();
        log.log_product("2026-03-05", &sample("Pizza", 250.0), 2.0).unwrap();
        let removed = log.undo_last("2026-03-05").unwrap();
        assert_eq!(removed, Some("Pizza".to_string()));
        assert_eq!(log.count("2026-03-05").unwrap(), 1);
    }

    #[test]
    fn test_undo_last_empty_day() {
        let log = DailyLog::open_in_memory().unwrap();
        let removed = log.undo_last("2026-03-05").unwrap();
        assert_eq!(removed, None);
    }
}

#[cfg(test)]
mod top_contributor_tests {
    use super::*;
    use crate::api::{Nutriments, Product};

    fn prod(name: &str, kcal: f64) -> Product {
        Product {
            code: "1".to_string(),
            product_name: Some(name.to_string()),
            brands: None, nutriscore_grade: None, nova_group: None,
            additives_tags: None,
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(kcal), fat_100g: Some(5.0),
                saturated_fat_100g: Some(1.0), sugars_100g: Some(8.0),
                salt_100g: Some(0.5), proteins_100g: Some(3.0),
                fiber_100g: Some(2.0), carbohydrates_100g: Some(20.0),
            }),
            ingredients_text: None, categories: None, allergens_tags: None, image_url: None, quantity: None, serving_size: None,
        }
    }

    #[test]
    fn test_top_contributor_single() {
        let log = DailyLog::open_in_memory().unwrap();
        log.log_product("2026-03-05", &prod("Apple", 52.0), 1.0).unwrap();
        let s = log.summary("2026-03-05").unwrap();
        assert_eq!(s.top_kcal_entry, Some(("Apple".to_string(), 52.0)));
    }

    #[test]
    fn test_top_contributor_picks_highest() {
        let log = DailyLog::open_in_memory().unwrap();
        log.log_product("2026-03-05", &prod("Apple", 52.0), 1.0).unwrap();
        log.log_product("2026-03-05", &prod("Pizza", 250.0), 2.0).unwrap();
        log.log_product("2026-03-05", &prod("Salad", 20.0), 1.0).unwrap();
        let s = log.summary("2026-03-05").unwrap();
        assert_eq!(s.top_kcal_entry, Some(("Pizza".to_string(), 500.0)));
    }

    #[test]
    fn test_top_contributor_empty() {
        let log = DailyLog::open_in_memory().unwrap();
        let s = log.summary("2026-03-05").unwrap();
        assert_eq!(s.top_kcal_entry, None);
    }
}

impl DailyLog {
    /// Count consecutive days with logged entries ending on `date` (inclusive).
    /// Returns 0 if no entries exist on the given date.
    pub fn streak(&self, date: &str) -> SqlResult<u32> {
        // First check if the given date has entries
        let has_entries: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM daily_log WHERE date = ?1)",
            rusqlite::params![date],
            |row| row.get(0),
        )?;
        if !has_entries {
            return Ok(0);
        }

        // Collect all distinct logged dates up to and including `date`, descending
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT date FROM daily_log WHERE date <= ?1 ORDER BY date DESC",
        )?;
        let dates: Vec<String> = stmt
            .query_map(rusqlite::params![date], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        if dates.is_empty() {
            return Ok(0);
        }

        // Walk backwards checking consecutive days
        let mut streak = 1u32;
        for i in 1..dates.len() {
            let prev = &dates[i - 1];
            let curr = &dates[i];
            // Parse dates and check they're exactly 1 day apart
            if is_previous_day(curr, prev) {
                streak += 1;
            } else {
                break;
            }
        }
        Ok(streak)
    }
}

/// Check if `earlier` is exactly one day before `later` (both YYYY-MM-DD).
fn is_previous_day(earlier: &str, later: &str) -> bool {
    let parse = |s: &str| -> Option<(u64, u64, u64)> {
        let parts: Vec<u64> = s.split('-').filter_map(|p| p.parse().ok()).collect();
        if parts.len() == 3 { Some((parts[0], parts[1], parts[2])) } else { None }
    };
    let Some((ey, em, ed)) = parse(earlier) else { return false };
    let Some((ly, lm, ld)) = parse(later) else { return false };
    let e_days = crate::ymd_to_days(ey, em, ed);
    let l_days = crate::ymd_to_days(ly, lm, ld);
    l_days == e_days + 1
}

#[cfg(test)]
mod streak_tests {
    use super::*;
    use crate::api::{Nutriments, Product};

    fn prod(name: &str) -> Product {
        Product {
            code: "1".to_string(),
            product_name: Some(name.to_string()),
            brands: None, nutriscore_grade: None, nova_group: None,
            additives_tags: None,
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(100.0), fat_100g: Some(5.0),
                saturated_fat_100g: Some(1.0), sugars_100g: Some(8.0),
                salt_100g: Some(0.5), proteins_100g: Some(3.0),
                fiber_100g: Some(2.0), carbohydrates_100g: Some(20.0),
            }),
            ingredients_text: None, categories: None, allergens_tags: None, image_url: None, quantity: None, serving_size: None,
        }
    }

    #[test]
    fn test_streak_no_entries() {
        let log = DailyLog::open_in_memory().unwrap();
        assert_eq!(log.streak("2026-03-10").unwrap(), 0);
    }

    #[test]
    fn test_streak_single_day() {
        let log = DailyLog::open_in_memory().unwrap();
        log.log_product("2026-03-10", &prod("Apple"), 1.0).unwrap();
        assert_eq!(log.streak("2026-03-10").unwrap(), 1);
    }

    #[test]
    fn test_streak_consecutive_days() {
        let log = DailyLog::open_in_memory().unwrap();
        log.log_product("2026-03-08", &prod("A"), 1.0).unwrap();
        log.log_product("2026-03-09", &prod("B"), 1.0).unwrap();
        log.log_product("2026-03-10", &prod("C"), 1.0).unwrap();
        assert_eq!(log.streak("2026-03-10").unwrap(), 3);
    }

    #[test]
    fn test_streak_broken_by_gap() {
        let log = DailyLog::open_in_memory().unwrap();
        log.log_product("2026-03-07", &prod("A"), 1.0).unwrap();
        // skip 2026-03-08
        log.log_product("2026-03-09", &prod("B"), 1.0).unwrap();
        log.log_product("2026-03-10", &prod("C"), 1.0).unwrap();
        assert_eq!(log.streak("2026-03-10").unwrap(), 2);
    }

    #[test]
    fn test_streak_cross_month() {
        let log = DailyLog::open_in_memory().unwrap();
        log.log_product("2026-02-27", &prod("A"), 1.0).unwrap();
        log.log_product("2026-02-28", &prod("B"), 1.0).unwrap();
        log.log_product("2026-03-01", &prod("C"), 1.0).unwrap();
        assert_eq!(log.streak("2026-03-01").unwrap(), 3);
    }

    #[test]
    fn test_streak_queried_midway() {
        let log = DailyLog::open_in_memory().unwrap();
        log.log_product("2026-03-08", &prod("A"), 1.0).unwrap();
        log.log_product("2026-03-09", &prod("B"), 1.0).unwrap();
        log.log_product("2026-03-10", &prod("C"), 1.0).unwrap();
        // Query streak as of 03-09
        assert_eq!(log.streak("2026-03-09").unwrap(), 2);
    }
}

/// Summary statistics for the daily log database.
#[derive(Debug)]
pub struct DailyStats {
    /// Total number of logged entries across all days.
    pub total_entries: u64,
    /// Number of distinct days with at least one entry.
    pub logged_days: u64,
    /// Earliest logged date (YYYY-MM-DD), if any.
    pub first_date: Option<String>,
    /// Latest logged date (YYYY-MM-DD), if any.
    pub last_date: Option<String>,
}

impl DailyLog {
    /// Aggregate statistics across the entire daily log.
    pub fn stats(&self) -> SqlResult<DailyStats> {
        let total_entries: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM daily_log",
            [],
            |row| row.get(0),
        )?;
        let logged_days: u64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT date) FROM daily_log",
            [],
            |row| row.get(0),
        )?;
        let first_date: Option<String> = self.conn.query_row(
            "SELECT MIN(date) FROM daily_log",
            [],
            |row| row.get(0),
        )?;
        let last_date: Option<String> = self.conn.query_row(
            "SELECT MAX(date) FROM daily_log",
            [],
            |row| row.get(0),
        )?;
        Ok(DailyStats {
            total_entries,
            logged_days,
            first_date,
            last_date,
        })
    }
}

#[cfg(test)]
mod daily_stats_tests {
    use super::*;
    use crate::api::{Product, Nutriments};

    fn test_product(name: &str) -> Product {
        Product {
            product_name: Some(name.to_string()),
            brands: None,
            nutriscore_grade: None,
            nova_group: None,
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(200.0),
                fat_100g: Some(10.0),
                saturated_fat_100g: Some(3.0),
                carbohydrates_100g: Some(25.0),
                sugars_100g: Some(5.0),
                salt_100g: Some(0.5),
                proteins_100g: Some(8.0),
                fiber_100g: Some(3.0),
            }),
            ingredients_text: None,
            additives_tags: None,
            code: String::new(),
            categories: None,
            allergens_tags: None, image_url: None, quantity: None, serving_size: None,
        }
    }

    #[test]
    fn test_stats_empty() {
        let log = DailyLog::open_in_memory().unwrap();
        let stats = log.stats().unwrap();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.logged_days, 0);
        assert!(stats.first_date.is_none());
        assert!(stats.last_date.is_none());
    }

    #[test]
    fn test_stats_with_entries() {
        let log = DailyLog::open_in_memory().unwrap();
        let p = test_product("Apple");
        log.log_product("2026-03-10", &p, 1.0).unwrap();
        log.log_product("2026-03-10", &p, 2.0).unwrap();
        log.log_product("2026-03-12", &p, 1.0).unwrap();
        let stats = log.stats().unwrap();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.logged_days, 2);
        assert_eq!(stats.first_date.as_deref(), Some("2026-03-10"));
        assert_eq!(stats.last_date.as_deref(), Some("2026-03-12"));
    }
}

/// A frequently logged product with its total log count and total servings.
#[derive(Debug, Clone, PartialEq)]
pub struct TopProduct {
    pub product_name: String,
    pub times_logged: u64,
    pub total_servings: f64,
}

impl DailyLog {
    /// Return the most frequently logged products, ordered by log count descending.
    pub fn top_products(&self, limit: u32) -> SqlResult<Vec<TopProduct>> {
        let mut stmt = self.conn.prepare(
            "SELECT product_name, COUNT(*) as cnt, SUM(servings) as total_srv
             FROM daily_log
             GROUP BY product_name
             ORDER BY cnt DESC, total_srv DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(TopProduct {
                product_name: row.get(0)?,
                times_logged: row.get(1)?,
                total_servings: row.get(2)?,
            })
        })?;
        rows.collect()
    }
}

#[cfg(test)]
mod top_products_tests {
    use super::*;
    use crate::api::{Nutriments, Product};

    fn make_product(code: &str, name: &str) -> Product {
        Product {
            code: code.to_string(),
            product_name: Some(name.to_string()),
            brands: None,
            nutriscore_grade: None,
            nova_group: None,
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(100.0),
                fat_100g: Some(5.0),
                saturated_fat_100g: Some(1.0),
                carbohydrates_100g: Some(15.0),
                sugars_100g: Some(5.0),
                salt_100g: Some(0.3),
                proteins_100g: Some(4.0),
                fiber_100g: Some(2.0),
            }),
            ingredients_text: None,
            categories: None,
            additives_tags: None,
            allergens_tags: None,
            image_url: None, quantity: None, serving_size: None,
        }
    }

    #[test]
    fn test_top_products_empty() {
        let log = DailyLog::open_in_memory().unwrap();
        let tops = log.top_products(5).unwrap();
        assert!(tops.is_empty());
    }

    #[test]
    fn test_top_products_order() {
        let log = DailyLog::open_in_memory().unwrap();
        let apple = make_product("1", "Apple");
        let bread = make_product("2", "Bread");
        let milk = make_product("3", "Milk");

        // Log apple 3 times, bread 1 time, milk 2 times
        log.log_product("2026-03-10", &apple, 1.0).unwrap();
        log.log_product("2026-03-10", &apple, 1.5).unwrap();
        log.log_product("2026-03-11", &apple, 1.0).unwrap();
        log.log_product("2026-03-10", &bread, 2.0).unwrap();
        log.log_product("2026-03-11", &milk, 1.0).unwrap();
        log.log_product("2026-03-12", &milk, 1.0).unwrap();

        let tops = log.top_products(5).unwrap();
        assert_eq!(tops.len(), 3);
        assert_eq!(tops[0].product_name, "Apple");
        assert_eq!(tops[0].times_logged, 3);
        assert!((tops[0].total_servings - 3.5).abs() < 0.01);
        assert_eq!(tops[1].product_name, "Milk");
        assert_eq!(tops[1].times_logged, 2);
        assert_eq!(tops[2].product_name, "Bread");
        assert_eq!(tops[2].times_logged, 1);
    }

    #[test]
    fn test_top_products_respects_limit() {
        let log = DailyLog::open_in_memory().unwrap();
        let a = make_product("1", "A");
        let b = make_product("2", "B");
        let c = make_product("3", "C");
        log.log_product("2026-03-10", &a, 1.0).unwrap();
        log.log_product("2026-03-10", &b, 1.0).unwrap();
        log.log_product("2026-03-10", &c, 1.0).unwrap();
        let tops = log.top_products(2).unwrap();
        assert_eq!(tops.len(), 2);
    }
}

/// Recommended Daily Values (based on a 2000 kcal diet, WHO/EU reference).
pub struct RecommendedDailyValues;

impl RecommendedDailyValues {
    pub const KCAL: f64 = 2000.0;
    pub const FAT: f64 = 70.0;        // g
    pub const SATURATED_FAT: f64 = 20.0; // g
    pub const CARBS: f64 = 260.0;      // g
    pub const SUGAR: f64 = 50.0;       // g (WHO free sugars limit)
    pub const PROTEIN: f64 = 50.0;     // g
    pub const SALT: f64 = 5.0;         // g (WHO limit)
    pub const FIBER: f64 = 25.0;       // g (WHO minimum)
}

/// Nutrient with its percentage of the recommended daily value.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RdvEntry {
    pub label: &'static str,
    pub value: f64,
    pub unit: &'static str,
    pub rdv: f64,
    pub pct: f64,
}

impl DailySummary {
    /// Calculate percentage of recommended daily values for all tracked nutrients.
    pub fn rdv_percentages(&self) -> Vec<RdvEntry> {
        let pairs: Vec<(&str, f64, &str, f64)> = vec![
            ("Energy", self.total_kcal, "kcal", RecommendedDailyValues::KCAL),
            ("Fat", self.total_fat, "g", RecommendedDailyValues::FAT),
            ("Sat. Fat", self.total_saturated_fat, "g", RecommendedDailyValues::SATURATED_FAT),
            ("Carbs", self.total_carbs, "g", RecommendedDailyValues::CARBS),
            ("Sugar", self.total_sugar, "g", RecommendedDailyValues::SUGAR),
            ("Protein", self.total_protein, "g", RecommendedDailyValues::PROTEIN),
            ("Salt", self.total_salt, "g", RecommendedDailyValues::SALT),
            ("Fiber", self.total_fiber, "g", RecommendedDailyValues::FIBER),
        ];
        pairs
            .into_iter()
            .map(|(label, value, unit, rdv)| {
                let pct = if rdv > 0.0 { value / rdv * 100.0 } else { 0.0 };
                RdvEntry { label, value, unit, rdv, pct }
            })
            .collect()
    }
}

#[cfg(test)]
mod rdv_tests {
    use super::*;

    #[test]
    fn test_rdv_percentages_empty() {
        let summary = DailySummary::default();
        let rdv = summary.rdv_percentages();
        assert_eq!(rdv.len(), 8);
        for entry in &rdv {
            assert!((entry.pct - 0.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_rdv_percentages_exact_targets() {
        let summary = DailySummary {
            entries: vec![],
            total_kcal: 2000.0,
            total_fat: 70.0,
            total_saturated_fat: 20.0,
            total_carbs: 260.0,
            total_sugar: 50.0,
            total_protein: 50.0,
            total_salt: 5.0,
            total_fiber: 25.0,
            top_kcal_entry: None,
        };
        let rdv = summary.rdv_percentages();
        for entry in &rdv {
            assert!(
                (entry.pct - 100.0).abs() < 0.01,
                "{} should be 100%, got {:.1}%",
                entry.label,
                entry.pct
            );
        }
    }

    #[test]
    fn test_rdv_percentages_half_values() {
        let summary = DailySummary {
            entries: vec![],
            total_kcal: 1000.0,
            total_fat: 35.0,
            total_saturated_fat: 10.0,
            total_carbs: 130.0,
            total_sugar: 25.0,
            total_protein: 25.0,
            total_salt: 2.5,
            total_fiber: 12.5,
            top_kcal_entry: None,
        };
        let rdv = summary.rdv_percentages();
        for entry in &rdv {
            assert!(
                (entry.pct - 50.0).abs() < 0.01,
                "{} should be 50%, got {:.1}%",
                entry.label,
                entry.pct
            );
        }
    }
}
