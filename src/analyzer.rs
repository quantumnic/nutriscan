use crate::api::Product;
use std::collections::HashMap;

/// Known problematic additives with risk descriptions.
fn additive_warnings() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("en:e150d", "Caramel color (sulfite ammonia) — potentially carcinogenic");
    m.insert("en:e950", "Acesulfame K — artificial sweetener, controversial");
    m.insert("en:e951", "Aspartame — artificial sweetener, controversial");
    m.insert("en:e621", "Monosodium glutamate — flavor enhancer, may cause headaches");
    m.insert("en:e102", "Tartrazine — azo dye, may cause hyperactivity");
    m.insert("en:e110", "Sunset Yellow — azo dye, may cause hyperactivity");
    m.insert("en:e122", "Azorubine — azo dye, may cause hyperactivity");
    m.insert("en:e211", "Sodium benzoate — preservative, may form benzene with vitamin C");
    m.insert("en:e250", "Sodium nitrite — preservative, potentially carcinogenic");
    m.insert("en:e320", "BHA — antioxidant, possible endocrine disruptor");
    m
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdditiveWarning {
    pub code: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NutriRating {
    Excellent,
    Good,
    Moderate,
    Poor,
    Bad,
    Unknown,
}

impl NutriRating {
    pub fn from_grade(grade: &str) -> Self {
        match grade.to_lowercase().as_str() {
            "a" => NutriRating::Excellent,
            "b" => NutriRating::Good,
            "c" => NutriRating::Moderate,
            "d" => NutriRating::Poor,
            "e" => NutriRating::Bad,
            _ => NutriRating::Unknown,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            NutriRating::Excellent => "A — Excellent",
            NutriRating::Good => "B — Good",
            NutriRating::Moderate => "C — Moderate",
            NutriRating::Poor => "D — Poor",
            NutriRating::Bad => "E — Bad",
            NutriRating::Unknown => "Unknown",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            NutriRating::Excellent => "🟢",
            NutriRating::Good => "🟡",
            NutriRating::Moderate => "🟠",
            NutriRating::Poor => "🔴",
            NutriRating::Bad => "⛔",
            NutriRating::Unknown => "❓",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NovaGroup {
    Unprocessed,
    ProcessedIngredients,
    Processed,
    UltraProcessed,
    Unknown,
}

impl NovaGroup {
    pub fn from_value(v: i32) -> Self {
        match v {
            1 => NovaGroup::Unprocessed,
            2 => NovaGroup::ProcessedIngredients,
            3 => NovaGroup::Processed,
            4 => NovaGroup::UltraProcessed,
            _ => NovaGroup::Unknown,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            NovaGroup::Unprocessed => "NOVA 1 — Unprocessed/minimally processed",
            NovaGroup::ProcessedIngredients => "NOVA 2 — Processed culinary ingredients",
            NovaGroup::Processed => "NOVA 3 — Processed foods",
            NovaGroup::UltraProcessed => "NOVA 4 — Ultra-processed",
            NovaGroup::Unknown => "Unknown",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            NovaGroup::Unprocessed => "🌿",
            NovaGroup::ProcessedIngredients => "🧂",
            NovaGroup::Processed => "🏭",
            NovaGroup::UltraProcessed => "⚠️",
            NovaGroup::Unknown => "❓",
        }
    }
}

#[derive(Debug)]
pub struct Analysis {
    pub product_name: String,
    pub brands: String,
    pub nutri_rating: NutriRating,
    pub nova: NovaGroup,
    pub warnings: Vec<AdditiveWarning>,
    pub product: Product,
}

pub fn analyze(product: &Product) -> Analysis {
    let nutri_rating = product
        .nutriscore_grade
        .as_deref()
        .map(NutriRating::from_grade)
        .unwrap_or(NutriRating::Unknown);

    let nova = product
        .nova_group
        .map(NovaGroup::from_value)
        .unwrap_or(NovaGroup::Unknown);

    let known = additive_warnings();
    let warnings: Vec<AdditiveWarning> = product
        .additives_tags
        .as_ref()
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| {
                    known.get(tag.as_str()).map(|desc| AdditiveWarning {
                        code: tag.clone(),
                        description: desc.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Analysis {
        product_name: product.product_name.clone().unwrap_or_else(|| "Unknown".into()),
        brands: product.brands.clone().unwrap_or_else(|| "Unknown".into()),
        nutri_rating,
        nova,
        warnings,
        product: product.clone(),
    }
}

#[allow(clippy::type_complexity)]
pub fn compare_products(a: &Product, b: &Product) -> Vec<(String, String, String)> {
    let mut diffs = Vec::new();
    let na = a.nutriments.as_ref();
    let nb = b.nutriments.as_ref();

    let fields: Vec<(&str, Box<dyn Fn(&crate::api::Nutriments) -> Option<f64>>)> = vec![
        ("Energy (kcal)", Box::new(|n: &crate::api::Nutriments| n.energy_kcal_100g)),
        ("Fat (g)", Box::new(|n| n.fat_100g)),
        ("Sugars (g)", Box::new(|n| n.sugars_100g)),
        ("Salt (g)", Box::new(|n| n.salt_100g)),
        ("Proteins (g)", Box::new(|n| n.proteins_100g)),
        ("Fiber (g)", Box::new(|n| n.fiber_100g)),
    ];

    for (label, getter) in &fields {
        let va = na.and_then(getter);
        let vb = nb.and_then(getter);
        diffs.push((
            label.to_string(),
            va.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "—".into()),
            vb.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "—".into()),
        ));
    }

    diffs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Nutriments, Product};

    fn make_product(grade: Option<&str>, nova: Option<i32>, additives: Vec<&str>) -> Product {
        Product {
            code: "1".into(),
            product_name: Some("Test".into()),
            brands: Some("Brand".into()),
            nutriscore_grade: grade.map(String::from),
            nova_group: nova,
            additives_tags: Some(additives.into_iter().map(String::from).collect()),
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(100.0),
                fat_100g: Some(5.0),
                saturated_fat_100g: Some(1.0),
                sugars_100g: Some(10.0),
                salt_100g: Some(0.5),
                proteins_100g: Some(3.0),
                fiber_100g: Some(2.0),
                carbohydrates_100g: Some(15.0),
            }),
            ingredients_text: None,
            categories: None,
            image_url: None,
        }
    }

    #[test]
    fn test_nutri_rating_from_grade() {
        assert_eq!(NutriRating::from_grade("a"), NutriRating::Excellent);
        assert_eq!(NutriRating::from_grade("E"), NutriRating::Bad);
        assert_eq!(NutriRating::from_grade("z"), NutriRating::Unknown);
    }

    #[test]
    fn test_nova_from_value() {
        assert_eq!(NovaGroup::from_value(1), NovaGroup::Unprocessed);
        assert_eq!(NovaGroup::from_value(4), NovaGroup::UltraProcessed);
        assert_eq!(NovaGroup::from_value(99), NovaGroup::Unknown);
    }

    #[test]
    fn test_analyze_with_warnings() {
        let p = make_product(Some("d"), Some(4), vec!["en:e150d", "en:e951"]);
        let a = analyze(&p);
        assert_eq!(a.nutri_rating, NutriRating::Poor);
        assert_eq!(a.nova, NovaGroup::UltraProcessed);
        assert_eq!(a.warnings.len(), 2);
    }

    #[test]
    fn test_analyze_no_warnings() {
        let p = make_product(Some("a"), Some(1), vec!["en:e300"]);
        let a = analyze(&p);
        assert!(a.warnings.is_empty());
    }

    #[test]
    fn test_compare_products() {
        let a = make_product(Some("a"), Some(1), vec![]);
        let mut b = make_product(Some("d"), Some(4), vec![]);
        b.nutriments.as_mut().unwrap().sugars_100g = Some(30.0);
        let diffs = compare_products(&a, &b);
        assert!(diffs.len() >= 5);
        let sugar_row = diffs.iter().find(|(l, _, _)| l == "Sugars (g)").unwrap();
        assert_eq!(sugar_row.1, "10.0");
        assert_eq!(sugar_row.2, "30.0");
    }
}
