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
    m.insert("en:e171", "Titanium dioxide — banned in EU as food additive");
    m.insert("en:e133", "Brilliant Blue — synthetic dye, limited studies");
    m.insert("en:e129", "Allura Red — azo dye, may cause hyperactivity");
    m.insert("en:e952", "Cyclamate — artificial sweetener, banned in some countries");
    m.insert("en:e955", "Sucralose — artificial sweetener, may affect gut microbiome");
    m
}

/// Common allergens detected from ingredients text.
const ALLERGEN_KEYWORDS: &[(&str, &str)] = &[
    ("milk", "Milk/Dairy"),
    ("lactose", "Milk/Dairy"),
    ("cream", "Milk/Dairy"),
    ("butter", "Milk/Dairy"),
    ("whey", "Milk/Dairy"),
    ("casein", "Milk/Dairy"),
    ("gluten", "Gluten"),
    ("wheat", "Wheat/Gluten"),
    ("barley", "Barley/Gluten"),
    ("soy", "Soy"),
    ("soya", "Soy"),
    ("peanut", "Peanuts"),
    ("almond", "Tree nuts"),
    ("hazelnut", "Tree nuts"),
    ("walnut", "Tree nuts"),
    ("cashew", "Tree nuts"),
    ("egg", "Eggs"),
    ("fish", "Fish"),
    ("shellfish", "Shellfish"),
    ("shrimp", "Shellfish"),
    ("sesame", "Sesame"),
    ("mustard", "Mustard"),
    ("celery", "Celery"),
    ("lupin", "Lupin"),
    ("sulphite", "Sulphites"),
    ("sulfite", "Sulphites"),
];

/// Detect potential allergens from ingredients text.
pub fn detect_allergens(ingredients: Option<&str>) -> Vec<String> {
    let Some(text) = ingredients else {
        return Vec::new();
    };
    let lower = text.to_lowercase();
    let mut found: Vec<String> = ALLERGEN_KEYWORDS
        .iter()
        .filter(|(keyword, _)| lower.contains(keyword))
        .map(|(_, allergen)| allergen.to_string())
        .collect();
    found.sort();
    found.dedup();
    found
}

/// Compute a simple 0-100 health score based on available data.
pub fn health_score(product: &Product) -> Option<u32> {
    let mut score: f64 = 50.0;
    let mut has_data = false;

    if let Some(ref grade) = product.nutriscore_grade {
        has_data = true;
        match grade.to_lowercase().as_str() {
            "a" => score += 25.0,
            "b" => score += 12.0,
            "c" => {}
            "d" => score -= 12.0,
            "e" => score -= 25.0,
            _ => {}
        }
    }

    if let Some(nova) = product.nova_group {
        has_data = true;
        match nova {
            1 => score += 15.0,
            2 => score += 5.0,
            3 => score -= 5.0,
            4 => score -= 15.0,
            _ => {}
        }
    }

    if let Some(ref n) = product.nutriments {
        if let Some(sugar) = n.sugars_100g {
            has_data = true;
            if sugar > 20.0 { score -= 5.0; }
            else if sugar < 5.0 { score += 3.0; }
        }
        if let Some(salt) = n.salt_100g {
            has_data = true;
            if salt > 1.5 { score -= 5.0; }
            else if salt < 0.3 { score += 2.0; }
        }
        if let Some(fiber) = n.fiber_100g {
            has_data = true;
            if fiber > 5.0 { score += 5.0; }
            else if fiber > 3.0 { score += 2.0; }
        }
        if let Some(protein) = n.proteins_100g {
            has_data = true;
            if protein > 10.0 { score += 3.0; }
        }
        if let Some(sat_fat) = n.saturated_fat_100g {
            has_data = true;
            if sat_fat > 5.0 { score -= 5.0; }
            else if sat_fat < 1.0 { score += 2.0; }
        }
    }

    if let Some(ref tags) = product.additives_tags {
        let known = additive_warnings();
        let bad_count = tags.iter().filter(|t| known.contains_key(t.as_str())).count();
        if bad_count > 0 {
            has_data = true;
            score -= (bad_count as f64) * 3.0;
        }
    }

    if !has_data {
        return None;
    }
    Some(score.clamp(0.0, 100.0) as u32)
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
    pub allergens: Vec<String>,
    pub health_score: Option<u32>,
    pub macro_balance: MacroBalance,
    pub energy_density: Option<EnergyDensity>,
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

    let allergens = detect_allergens(product.ingredients_text.as_deref());
    let score = health_score(product);

    let macro_balance = assess_macro_balance(product);
    let energy_density = classify_energy_density(product);

    Analysis {
        product_name: product.product_name.clone().unwrap_or_else(|| "Unknown".into()),
        brands: product.brands.clone().unwrap_or_else(|| "Unknown".into()),
        nutri_rating,
        nova,
        warnings,
        allergens,
        health_score: score,
        macro_balance,
        energy_density,
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
        ("Sat. Fat (g)", Box::new(|n| n.saturated_fat_100g)),
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

/// Macro-nutrient balance assessment.
#[derive(Debug, Clone, PartialEq)]
pub enum MacroBalance {
    Balanced,
    HighIn(String),
    Unknown,
}


/// Energy density classification based on kcal per 100g.
#[derive(Debug, Clone, PartialEq)]
pub enum EnergyDensity {
    /// < 60 kcal/100g (most fruits, vegetables, broth)
    VeryLow,
    /// 60–150 kcal/100g (cooked grains, lean meat)
    Low,
    /// 150–400 kcal/100g (bread, cheese, meat)
    Medium,
    /// > 400 kcal/100g (nuts, oils, chocolate)
    High,
}

impl EnergyDensity {
    pub fn label(&self) -> &'static str {
        match self {
            EnergyDensity::VeryLow => "Very low (<60 kcal/100g)",
            EnergyDensity::Low => "Low (60–150 kcal/100g)",
            EnergyDensity::Medium => "Medium (150–400 kcal/100g)",
            EnergyDensity::High => "High (>400 kcal/100g)",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            EnergyDensity::VeryLow => "🟢",
            EnergyDensity::Low => "🟡",
            EnergyDensity::Medium => "🟠",
            EnergyDensity::High => "🔴",
        }
    }
}

/// Classify energy density from nutriments.
pub fn classify_energy_density(product: &Product) -> Option<EnergyDensity> {
    let kcal = product.nutriments.as_ref()?.energy_kcal_100g?;
    Some(match kcal {
        x if x < 60.0 => EnergyDensity::VeryLow,
        x if x < 150.0 => EnergyDensity::Low,
        x if x < 400.0 => EnergyDensity::Medium,
        _ => EnergyDensity::High,
    })
}

/// Assess macro-nutrient balance from nutriments.
pub fn assess_macro_balance(product: &Product) -> MacroBalance {
    let n = match &product.nutriments {
        Some(n) => n,
        None => return MacroBalance::Unknown,
    };
    let fat = n.fat_100g.unwrap_or(0.0);
    let carbs = n.carbohydrates_100g.unwrap_or(0.0);
    let protein = n.proteins_100g.unwrap_or(0.0);
    let total = fat + carbs + protein;
    if total < 1.0 { return MacroBalance::Unknown; }
    let fat_pct = fat / total * 100.0;
    let carb_pct = carbs / total * 100.0;
    let protein_pct = protein / total * 100.0;
    if fat_pct > 60.0 { MacroBalance::HighIn("fat".to_string()) }
    else if carb_pct > 75.0 { MacroBalance::HighIn("carbohydrates".to_string()) }
    else if protein_pct > 60.0 { MacroBalance::HighIn("protein".to_string()) }
    else { MacroBalance::Balanced }
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
    fn test_detect_allergens_found() {
        let allergens = detect_allergens(Some("water, milk, wheat flour, soy lecithin"));
        assert!(allergens.contains(&"Milk/Dairy".to_string()));
        assert!(allergens.contains(&"Wheat/Gluten".to_string()));
        assert!(allergens.contains(&"Soy".to_string()));
    }

    #[test]
    fn test_detect_allergens_none() {
        let allergens = detect_allergens(Some("water, sugar, salt"));
        assert!(allergens.is_empty());
    }

    #[test]
    fn test_detect_allergens_no_text() {
        let allergens = detect_allergens(None);
        assert!(allergens.is_empty());
    }

    #[test]
    fn test_health_score_excellent() {
        let p = make_product(Some("a"), Some(1), vec![]);
        let score = health_score(&p).unwrap();
        assert!(score >= 80, "Expected high score, got {}", score);
    }

    #[test]
    fn test_health_score_poor() {
        let p = make_product(Some("e"), Some(4), vec!["en:e150d", "en:e951"]);
        let score = health_score(&p).unwrap();
        assert!(score <= 30, "Expected low score, got {}", score);
    }

    #[test]
    fn test_health_score_no_data() {
        let p = Product {
            code: "1".into(),
            product_name: None,
            brands: None,
            nutriscore_grade: None,
            nova_group: None,
            additives_tags: None,
            nutriments: None,
            ingredients_text: None,
            categories: None,
            image_url: None,
        };
        assert!(health_score(&p).is_none());
    }

    #[test]
    fn test_analyze_includes_allergens() {
        let mut p = make_product(Some("b"), Some(2), vec![]);
        p.ingredients_text = Some("water, milk, egg".to_string());
        let a = analyze(&p);
        assert!(a.allergens.contains(&"Milk/Dairy".to_string()));
        assert!(a.allergens.contains(&"Eggs".to_string()));
        assert!(a.health_score.is_some());
    }
    #[test]
    fn test_compare_products() {
        let a = make_product(Some("a"), Some(1), vec![]);
        let mut b = make_product(Some("d"), Some(4), vec![]);
        b.nutriments.as_mut().unwrap().sugars_100g = Some(30.0);
        let diffs = compare_products(&a, &b);
        assert!(diffs.len() >= 6);
        let sugar_row = diffs.iter().find(|(l, _, _)| l == "Sugars (g)").unwrap();
        assert_eq!(sugar_row.1, "10.0");
        assert_eq!(sugar_row.2, "30.0");
    }

    #[test]
    fn test_health_score_saturated_fat_penalty() {
        let mut p = make_product(Some("c"), Some(3), vec![]);
        p.nutriments.as_mut().unwrap().saturated_fat_100g = Some(8.0);
        let score = health_score(&p).unwrap();
        let baseline = health_score(&make_product(Some("c"), Some(3), vec![])).unwrap();
        assert!(score < baseline, "sat fat penalty: {} vs {}", score, baseline);
    }

    #[test]
    fn test_health_score_low_sat_fat_bonus() {
        let mut p = make_product(Some("c"), Some(3), vec![]);
        p.nutriments.as_mut().unwrap().saturated_fat_100g = Some(0.5);
        let score = health_score(&p).unwrap();
        let baseline = health_score(&make_product(Some("c"), Some(3), vec![])).unwrap();
        assert!(score > baseline, "low sat fat bonus: {} vs {}", score, baseline);
    }

    #[test]
    fn test_health_score_clamps_max() {
        let mut p = make_product(Some("a"), Some(1), vec![]);
        let n = p.nutriments.as_mut().unwrap();
        n.sugars_100g = Some(1.0);
        n.salt_100g = Some(0.1);
        n.fiber_100g = Some(10.0);
        n.proteins_100g = Some(20.0);
        n.saturated_fat_100g = Some(0.1);
        let score = health_score(&p).unwrap();
        assert!(score <= 100);
    }

    #[test]
    fn test_health_score_clamps_min() {
        let p = make_product(Some("e"), Some(4), vec![
            "en:e150d", "en:e950", "en:e951", "en:e621", "en:e102",
            "en:e110", "en:e122", "en:e211", "en:e250", "en:e320",
        ]);
        let score = health_score(&p).unwrap();
        assert_eq!(score, 0);
    }

    #[test]
    fn test_macro_balance_balanced() {
        let p = make_product(Some("b"), Some(2), vec![]);
        assert_eq!(assess_macro_balance(&p), MacroBalance::Balanced);
    }

    #[test]
    fn test_macro_balance_high_fat() {
        let mut p = make_product(Some("d"), Some(3), vec![]);
        let n = p.nutriments.as_mut().unwrap();
        n.fat_100g = Some(80.0);
        n.carbohydrates_100g = Some(5.0);
        n.proteins_100g = Some(2.0);
        assert_eq!(assess_macro_balance(&p), MacroBalance::HighIn("fat".to_string()));
    }

    #[test]
    fn test_macro_balance_no_data() {
        let p = Product {
            code: "1".into(), product_name: None, brands: None,
            nutriscore_grade: None, nova_group: None, additives_tags: None,
            nutriments: None, ingredients_text: None, categories: None, image_url: None,
        };
        assert_eq!(assess_macro_balance(&p), MacroBalance::Unknown);
    }

    #[test]
    fn test_allergen_dedup() {
        let allergens = detect_allergens(Some("milk, lactose, cream"));
        let dairy_count = allergens.iter().filter(|a| a.as_str() == "Milk/Dairy").count();
        assert_eq!(dairy_count, 1);
    }

    #[test]
    fn test_compare_missing_nutriments() {
        let a = make_product(Some("a"), Some(1), vec![]);
        let b = Product {
            code: "2".into(), product_name: Some("Empty".into()), brands: None,
            nutriscore_grade: None, nova_group: None, additives_tags: None,
            nutriments: None, ingredients_text: None, categories: None, image_url: None,
        };
        let diffs = compare_products(&a, &b);
        for (_, _, vb) in &diffs { assert_eq!(vb, "\u{2014}"); }
    }

    #[test]
    fn test_additive_warning_all_known() {
        let p = make_product(Some("e"), Some(4), vec![
            "en:e150d", "en:e950", "en:e951", "en:e621", "en:e102",
        ]);
        let a = analyze(&p);
        assert_eq!(a.warnings.len(), 5);
    }


    #[test]
    fn test_energy_density_very_low() {
        let mut p = make_product(Some("a"), Some(1), vec![]);
        p.nutriments.as_mut().unwrap().energy_kcal_100g = Some(30.0);
        assert_eq!(classify_energy_density(&p), Some(EnergyDensity::VeryLow));
    }

    #[test]
    fn test_energy_density_low() {
        let mut p = make_product(Some("b"), Some(2), vec![]);
        p.nutriments.as_mut().unwrap().energy_kcal_100g = Some(100.0);
        assert_eq!(classify_energy_density(&p), Some(EnergyDensity::Low));
    }

    #[test]
    fn test_energy_density_medium() {
        let p = make_product(Some("c"), Some(3), vec![]);  // 100 kcal default
        // default is 100 kcal -> Low, so set to 250
        let mut p2 = p;
        p2.nutriments.as_mut().unwrap().energy_kcal_100g = Some(250.0);
        assert_eq!(classify_energy_density(&p2), Some(EnergyDensity::Medium));
    }

    #[test]
    fn test_energy_density_high() {
        let mut p = make_product(Some("d"), Some(4), vec![]);
        p.nutriments.as_mut().unwrap().energy_kcal_100g = Some(550.0);
        assert_eq!(classify_energy_density(&p), Some(EnergyDensity::High));
    }

    #[test]
    fn test_energy_density_none_without_nutriments() {
        let p = Product {
            code: "1".into(), product_name: None, brands: None,
            nutriscore_grade: None, nova_group: None, additives_tags: None,
            nutriments: None, ingredients_text: None, categories: None, image_url: None,
        };
        assert_eq!(classify_energy_density(&p), None);
    }

    #[test]
    fn test_energy_density_boundary_60() {
        let mut p = make_product(Some("a"), Some(1), vec![]);
        p.nutriments.as_mut().unwrap().energy_kcal_100g = Some(60.0);
        assert_eq!(classify_energy_density(&p), Some(EnergyDensity::Low));
    }

    #[test]
    fn test_analysis_includes_energy_density() {
        let p = make_product(Some("b"), Some(2), vec![]);
        let a = analyze(&p);
        assert!(a.energy_density.is_some());
    }
}
