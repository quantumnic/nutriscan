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
pub enum HealthRating {
    Excellent,
    Good,
    Moderate,
    Poor,
    Bad,
}

impl HealthRating {
    pub fn from_score(score: u32) -> Self {
        match score {
            80..=100 => HealthRating::Excellent,
            60..=79 => HealthRating::Good,
            40..=59 => HealthRating::Moderate,
            20..=39 => HealthRating::Poor,
            _ => HealthRating::Bad,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            HealthRating::Excellent => "Excellent",
            HealthRating::Good => "Good",
            HealthRating::Moderate => "Moderate",
            HealthRating::Poor => "Poor",
            HealthRating::Bad => "Bad",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            HealthRating::Excellent => "💚",
            HealthRating::Good => "💛",
            HealthRating::Moderate => "🧡",
            HealthRating::Poor => "❤️",
            HealthRating::Bad => "🖤",
        }
    }
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
    pub health_rating: Option<HealthRating>,
    pub macro_balance: MacroBalance,
    pub energy_density: Option<EnergyDensity>,
    pub protein_density: Option<ProteinDensity>,
    pub fiber_density: Option<FiberDensity>,
    pub sugar_density: Option<SugarDensity>,
    pub sat_fat_density: Option<SatFatDensity>,
    pub salt_density: Option<SaltDensity>,
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
    let protein_density = classify_protein_density(product);
    let fiber_density = classify_fiber_density(product);
    let sugar_density = classify_sugar_density(product);
    let sat_fat_density = classify_sat_fat_density(product);
    let salt_density = classify_salt_density(product);

    Analysis {
        product_name: product.product_name.clone().unwrap_or_else(|| "Unknown".into()),
        brands: product.brands.clone().unwrap_or_else(|| "Unknown".into()),
        nutri_rating,
        nova,
        warnings,
        allergens,
        health_score: score,
        health_rating: score.map(HealthRating::from_score),
        macro_balance,
        energy_density,
        protein_density,
        fiber_density,
        sugar_density,
        sat_fat_density,
        salt_density,
        product: product.clone(),
    }
}

/// Winner hint for a comparison row.
#[derive(Debug, Clone, PartialEq)]
pub enum CompareWinner {
    /// Product A is better (or equal).
    A,
    /// Product B is better.
    B,
    /// Tied or not enough data.
    Tie,
}

#[allow(clippy::type_complexity)]
pub fn compare_products(a: &Product, b: &Product) -> Vec<(String, String, String, CompareWinner)> {
    let mut diffs = Vec::new();
    let na = a.nutriments.as_ref();
    let nb = b.nutriments.as_ref();

    // (label, getter, higher_is_better)
    let fields: Vec<(&str, Box<dyn Fn(&crate::api::Nutriments) -> Option<f64>>, bool)> = vec![
        ("Energy (kcal)", Box::new(|n: &crate::api::Nutriments| n.energy_kcal_100g), false),
        ("Fat (g)", Box::new(|n| n.fat_100g), false),
        ("Carbs (g)", Box::new(|n| n.carbohydrates_100g), false),
        ("Sugars (g)", Box::new(|n| n.sugars_100g), false),
        ("Salt (g)", Box::new(|n| n.salt_100g), false),
        ("Proteins (g)", Box::new(|n| n.proteins_100g), true),
        ("Fiber (g)", Box::new(|n| n.fiber_100g), true),
        ("Sat. Fat (g)", Box::new(|n| n.saturated_fat_100g), false),
    ];

    for (label, getter, higher_is_better) in &fields {
        let va = na.and_then(getter);
        let vb = nb.and_then(getter);
        let winner = match (va, vb) {
            (Some(x), Some(y)) => {
                let (better_a, better_b) = if *higher_is_better {
                    (x > y, y > x)
                } else {
                    (x < y, y < x)
                };
                if better_a {
                    CompareWinner::A
                } else if better_b {
                    CompareWinner::B
                } else {
                    CompareWinner::Tie
                }
            }
            _ => CompareWinner::Tie,
        };
        diffs.push((
            label.to_string(),
            va.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "—".into()),
            vb.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "—".into()),
            winner,
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

/// Protein density classification based on grams of protein per 100 kcal.
/// Higher values indicate more satiating, protein-rich foods.
#[derive(Debug, Clone, PartialEq)]
pub enum ProteinDensity {
    /// < 5 g protein per 100 kcal
    Low,
    /// 5-10 g protein per 100 kcal
    Moderate,
    /// 10-20 g protein per 100 kcal
    High,
    /// > 20 g protein per 100 kcal (e.g. chicken breast, egg whites)
    VeryHigh,
}

impl ProteinDensity {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::High => "High",
            Self::VeryHigh => "Very high",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Low => "🔻",
            Self::Moderate => "➖",
            Self::High => "💪",
            Self::VeryHigh => "🏆",
        }
    }
}

/// Calculate protein density: grams of protein per 100 kcal.
/// Returns None if energy or protein data is missing or energy is near zero.
pub fn classify_protein_density(product: &Product) -> Option<ProteinDensity> {
    let n = product.nutriments.as_ref()?;
    let kcal = n.energy_kcal_100g?;
    let protein = n.proteins_100g?;
    if kcal < 1.0 {
        return None;
    }
    let per_100 = protein / kcal * 100.0;
    Some(match per_100 {
        x if x < 5.0 => ProteinDensity::Low,
        x if x < 10.0 => ProteinDensity::Moderate,
        x if x < 20.0 => ProteinDensity::High,
        _ => ProteinDensity::VeryHigh,
    })
}

/// Fiber density classification based on grams of fiber per 100 kcal.
/// Higher values indicate more satiating, gut-healthy foods.
#[derive(Debug, Clone, PartialEq)]
pub enum FiberDensity {
    /// < 1 g fiber per 100 kcal
    Low,
    /// 1-3 g fiber per 100 kcal
    Moderate,
    /// 3-6 g fiber per 100 kcal
    High,
    /// > 6 g fiber per 100 kcal (e.g. beans, berries, bran)
    VeryHigh,
}

impl FiberDensity {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::High => "High",
            Self::VeryHigh => "Very high",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Low => "🔻",
            Self::Moderate => "➖",
            Self::High => "🌿",
            Self::VeryHigh => "🥦",
        }
    }
}

/// Calculate fiber density: grams of fiber per 100 kcal.
/// Returns None if energy or fiber data is missing or energy is near zero.
pub fn classify_fiber_density(product: &Product) -> Option<FiberDensity> {
    let n = product.nutriments.as_ref()?;
    let kcal = n.energy_kcal_100g?;
    let fiber = n.fiber_100g?;
    if kcal < 1.0 {
        return None;
    }
    let per_100 = fiber / kcal * 100.0;
    Some(match per_100 {
        x if x < 1.0 => FiberDensity::Low,
        x if x < 3.0 => FiberDensity::Moderate,
        x if x < 6.0 => FiberDensity::High,
        _ => FiberDensity::VeryHigh,
    })
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
        assert!(diffs.len() >= 7);
        let sugar_row = diffs.iter().find(|(l, _, _, _)| l == "Sugars (g)").unwrap();
        assert_eq!(sugar_row.1, "10.0");
        assert_eq!(sugar_row.2, "30.0");
        // Lower sugar is better, so product A should win
        assert_eq!(sugar_row.3, CompareWinner::A);
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
    fn test_health_rating_from_score() {
        assert_eq!(HealthRating::from_score(90), HealthRating::Excellent);
        assert_eq!(HealthRating::from_score(80), HealthRating::Excellent);
        assert_eq!(HealthRating::from_score(70), HealthRating::Good);
        assert_eq!(HealthRating::from_score(60), HealthRating::Good);
        assert_eq!(HealthRating::from_score(50), HealthRating::Moderate);
        assert_eq!(HealthRating::from_score(40), HealthRating::Moderate);
        assert_eq!(HealthRating::from_score(30), HealthRating::Poor);
        assert_eq!(HealthRating::from_score(20), HealthRating::Poor);
        assert_eq!(HealthRating::from_score(10), HealthRating::Bad);
        assert_eq!(HealthRating::from_score(0), HealthRating::Bad);
    }

    #[test]
    fn test_health_rating_labels_and_emojis() {
        assert_eq!(HealthRating::Excellent.label(), "Excellent");
        assert_eq!(HealthRating::Excellent.emoji(), "💚");
        assert_eq!(HealthRating::Bad.label(), "Bad");
        assert_eq!(HealthRating::Bad.emoji(), "🖤");
    }

    #[test]
    fn test_analysis_includes_health_rating() {
        let p = make_product(Some("a"), Some(1), vec![]);
        let a = analyze(&p);
        assert!(a.health_rating.is_some());
        assert_eq!(a.health_rating.unwrap(), HealthRating::Excellent);
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
        for (_, _, vb, w) in &diffs {
            assert_eq!(vb, "\u{2014}");
            assert_eq!(*w, CompareWinner::Tie);
        }
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
    fn test_winner_lower_is_better() {
        let mut a = make_product(Some("a"), Some(1), vec![]);
        let mut b = make_product(Some("a"), Some(1), vec![]);
        a.nutriments.as_mut().unwrap().fat_100g = Some(5.0);
        b.nutriments.as_mut().unwrap().fat_100g = Some(15.0);
        let diffs = compare_products(&a, &b);
        let fat_row = diffs.iter().find(|(l, _, _, _)| l == "Fat (g)").unwrap();
        assert_eq!(fat_row.3, CompareWinner::A);
    }

    #[test]
    fn test_winner_higher_is_better() {
        let mut a = make_product(Some("a"), Some(1), vec![]);
        let mut b = make_product(Some("a"), Some(1), vec![]);
        a.nutriments.as_mut().unwrap().proteins_100g = Some(5.0);
        b.nutriments.as_mut().unwrap().proteins_100g = Some(25.0);
        let diffs = compare_products(&a, &b);
        let prot_row = diffs.iter().find(|(l, _, _, _)| l == "Proteins (g)").unwrap();
        assert_eq!(prot_row.3, CompareWinner::B);
    }

    #[test]
    fn test_winner_tie() {
        let a = make_product(Some("a"), Some(1), vec![]);
        let b = make_product(Some("a"), Some(1), vec![]);
        let diffs = compare_products(&a, &b);
        for (_, _, _, w) in &diffs {
            assert_eq!(*w, CompareWinner::Tie);
        }
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

    #[test]
    fn test_classify_protein_density_low() {
        let mut p = make_product(None, None, vec![]);
        // 3g protein per 100kcal = 3.0 per 100 => Low
        p.nutriments = Some(Nutriments {
            energy_kcal_100g: Some(100.0),
            proteins_100g: Some(3.0),
            ..Default::default()
        });
        assert_eq!(classify_protein_density(&p), Some(ProteinDensity::Low));
    }

    #[test]
    fn test_classify_protein_density_high() {
        let mut p = make_product(None, None, vec![]);
        // 15g protein per 100kcal = 15.0 per 100 => High
        p.nutriments = Some(Nutriments {
            energy_kcal_100g: Some(100.0),
            proteins_100g: Some(15.0),
            ..Default::default()
        });
        assert_eq!(classify_protein_density(&p), Some(ProteinDensity::High));
    }

    #[test]
    fn test_classify_protein_density_very_high() {
        let mut p = make_product(None, None, vec![]);
        // 25g protein per 100kcal => VeryHigh
        p.nutriments = Some(Nutriments {
            energy_kcal_100g: Some(100.0),
            proteins_100g: Some(25.0),
            ..Default::default()
        });
        assert_eq!(classify_protein_density(&p), Some(ProteinDensity::VeryHigh));
    }

    #[test]
    fn test_classify_protein_density_none_without_data() {
        let mut p = make_product(None, None, vec![]);
        p.nutriments = None;
        assert_eq!(classify_protein_density(&p), None);
    }

    #[test]
    fn test_analysis_includes_protein_density() {
        let p = make_product(Some("b"), Some(2), vec![]);
        let a = analyze(&p);
        assert!(a.protein_density.is_some());
    }
    #[test]
    fn test_classify_fiber_density_low() {
        let mut p = make_product(None, None, vec![]);
        // 0.5g fiber per 100kcal = 0.5 per 100 => Low
        p.nutriments = Some(Nutriments {
            energy_kcal_100g: Some(100.0),
            fiber_100g: Some(0.5),
            ..Default::default()
        });
        assert_eq!(classify_fiber_density(&p), Some(FiberDensity::Low));
    }

    #[test]
    fn test_classify_fiber_density_moderate() {
        let mut p = make_product(None, None, vec![]);
        p.nutriments = Some(Nutriments {
            energy_kcal_100g: Some(100.0),
            fiber_100g: Some(2.0),
            ..Default::default()
        });
        assert_eq!(classify_fiber_density(&p), Some(FiberDensity::Moderate));
    }

    #[test]
    fn test_classify_fiber_density_high() {
        let mut p = make_product(None, None, vec![]);
        p.nutriments = Some(Nutriments {
            energy_kcal_100g: Some(100.0),
            fiber_100g: Some(4.0),
            ..Default::default()
        });
        assert_eq!(classify_fiber_density(&p), Some(FiberDensity::High));
    }

    #[test]
    fn test_classify_fiber_density_very_high() {
        let mut p = make_product(None, None, vec![]);
        // 8g fiber per 100kcal => VeryHigh
        p.nutriments = Some(Nutriments {
            energy_kcal_100g: Some(100.0),
            fiber_100g: Some(8.0),
            ..Default::default()
        });
        assert_eq!(classify_fiber_density(&p), Some(FiberDensity::VeryHigh));
    }

    #[test]
    fn test_classify_fiber_density_none_without_data() {
        let mut p = make_product(None, None, vec![]);
        p.nutriments = None;
        assert_eq!(classify_fiber_density(&p), None);
    }

    #[test]
    fn test_analysis_includes_fiber_density() {
        let p = make_product(Some("b"), Some(2), vec![]);
        let a = analyze(&p);
        assert!(a.fiber_density.is_some());
    }


}

/// Sugar density classification based on grams of sugar per 100 kcal.
/// Lower values indicate healthier choices regarding added/free sugars.
#[derive(Debug, Clone, PartialEq)]
pub enum SugarDensity {
    /// < 5 g sugar per 100 kcal
    Low,
    /// 5-10 g sugar per 100 kcal
    Moderate,
    /// 10-20 g sugar per 100 kcal
    High,
    /// > 20 g sugar per 100 kcal (e.g. candy, soft drinks, jams)
    VeryHigh,
}

impl SugarDensity {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::High => "High",
            Self::VeryHigh => "Very high",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Low => "🟢",
            Self::Moderate => "🟡",
            Self::High => "🟠",
            Self::VeryHigh => "🔴",
        }
    }
}

/// Calculate sugar density: grams of sugar per 100 kcal.
/// Returns None if energy or sugar data is missing or energy is near zero.
pub fn classify_sugar_density(product: &Product) -> Option<SugarDensity> {
    let n = product.nutriments.as_ref()?;
    let kcal = n.energy_kcal_100g?;
    let sugar = n.sugars_100g?;
    if kcal < 1.0 {
        return None;
    }
    let per_100 = sugar / kcal * 100.0;
    Some(match per_100 {
        x if x < 5.0 => SugarDensity::Low,
        x if x < 10.0 => SugarDensity::Moderate,
        x if x < 20.0 => SugarDensity::High,
        _ => SugarDensity::VeryHigh,
    })
}

#[cfg(test)]
mod sugar_density_tests {
    use super::*;
    use crate::api::{Nutriments, Product};

    fn make_product_sugar(kcal: f64, sugar: f64) -> Product {
        Product {
            code: "1".into(),
            product_name: Some("Test".into()),
            brands: None,
            nutriscore_grade: None,
            nova_group: None,
            additives_tags: None,
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(kcal),
                sugars_100g: Some(sugar),
                ..Default::default()
            }),
            ingredients_text: None,
            categories: None,
            image_url: None,
        }
    }

    #[test]
    fn test_classify_sugar_density_low() {
        // 3g sugar per 100kcal = 3.0 per 100 => Low
        let p = make_product_sugar(100.0, 3.0);
        assert_eq!(classify_sugar_density(&p), Some(SugarDensity::Low));
    }

    #[test]
    fn test_classify_sugar_density_moderate() {
        // 7g sugar per 100kcal => Moderate
        let p = make_product_sugar(100.0, 7.0);
        assert_eq!(classify_sugar_density(&p), Some(SugarDensity::Moderate));
    }

    #[test]
    fn test_classify_sugar_density_high() {
        // 15g sugar per 100kcal => High
        let p = make_product_sugar(100.0, 15.0);
        assert_eq!(classify_sugar_density(&p), Some(SugarDensity::High));
    }

    #[test]
    fn test_classify_sugar_density_very_high() {
        // 25g sugar per 100kcal => VeryHigh
        let p = make_product_sugar(100.0, 25.0);
        assert_eq!(classify_sugar_density(&p), Some(SugarDensity::VeryHigh));
    }

    #[test]
    fn test_classify_sugar_density_none_without_data() {
        let p = Product {
            code: "1".into(), product_name: None, brands: None,
            nutriscore_grade: None, nova_group: None, additives_tags: None,
            nutriments: None, ingredients_text: None, categories: None, image_url: None,
        };
        assert_eq!(classify_sugar_density(&p), None);
    }

    #[test]
    fn test_classify_sugar_density_zero_kcal() {
        let p = make_product_sugar(0.0, 5.0);
        assert_eq!(classify_sugar_density(&p), None);
    }

    #[test]
    fn test_analysis_includes_sugar_density() {
        let mut p = make_product_sugar(100.0, 10.0);
        p.nutriscore_grade = Some("b".into());
        p.nova_group = Some(2);
        let a = analyze(&p);
        assert!(a.sugar_density.is_some());
    }
}

/// Saturated fat density classification based on grams of saturated fat per 100 kcal.
/// Lower values indicate heart-healthier choices.
#[derive(Debug, Clone, PartialEq)]
pub enum SatFatDensity {
    /// < 1 g sat fat per 100 kcal
    Low,
    /// 1-3 g sat fat per 100 kcal
    Moderate,
    /// 3-6 g sat fat per 100 kcal
    High,
    /// > 6 g sat fat per 100 kcal (e.g. butter, cream, coconut oil)
    VeryHigh,
}

impl SatFatDensity {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::High => "High",
            Self::VeryHigh => "Very high",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Low => "🟢",
            Self::Moderate => "🟡",
            Self::High => "🟠",
            Self::VeryHigh => "🔴",
        }
    }
}

/// Calculate saturated fat density: grams of saturated fat per 100 kcal.
/// Returns None if energy or saturated fat data is missing or energy is near zero.
pub fn classify_sat_fat_density(product: &Product) -> Option<SatFatDensity> {
    let n = product.nutriments.as_ref()?;
    let kcal = n.energy_kcal_100g?;
    let sat_fat = n.saturated_fat_100g?;
    if kcal < 1.0 {
        return None;
    }
    let per_100 = sat_fat / kcal * 100.0;
    Some(match per_100 {
        x if x < 1.0 => SatFatDensity::Low,
        x if x < 3.0 => SatFatDensity::Moderate,
        x if x < 6.0 => SatFatDensity::High,
        _ => SatFatDensity::VeryHigh,
    })
}

#[cfg(test)]
mod sat_fat_density_tests {
    use super::*;
    use crate::api::{Nutriments, Product};

    fn make_product_sat_fat(kcal: f64, sat_fat: f64) -> Product {
        Product {
            code: "1".into(),
            product_name: Some("Test".into()),
            brands: None,
            nutriscore_grade: None,
            nova_group: None,
            additives_tags: None,
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(kcal),
                saturated_fat_100g: Some(sat_fat),
                ..Default::default()
            }),
            ingredients_text: None,
            categories: None,
            image_url: None,
        }
    }

    #[test]
    fn test_classify_sat_fat_density_low() {
        // 0.5g sat fat per 100kcal => Low
        let p = make_product_sat_fat(100.0, 0.5);
        assert_eq!(classify_sat_fat_density(&p), Some(SatFatDensity::Low));
    }

    #[test]
    fn test_classify_sat_fat_density_moderate() {
        // 2g sat fat per 100kcal => Moderate
        let p = make_product_sat_fat(100.0, 2.0);
        assert_eq!(classify_sat_fat_density(&p), Some(SatFatDensity::Moderate));
    }

    #[test]
    fn test_classify_sat_fat_density_high() {
        // 4g sat fat per 100kcal => High
        let p = make_product_sat_fat(100.0, 4.0);
        assert_eq!(classify_sat_fat_density(&p), Some(SatFatDensity::High));
    }

    #[test]
    fn test_classify_sat_fat_density_very_high() {
        // 8g sat fat per 100kcal => VeryHigh
        let p = make_product_sat_fat(100.0, 8.0);
        assert_eq!(classify_sat_fat_density(&p), Some(SatFatDensity::VeryHigh));
    }

    #[test]
    fn test_classify_sat_fat_density_none_without_data() {
        let p = Product {
            code: "1".into(), product_name: None, brands: None,
            nutriscore_grade: None, nova_group: None, additives_tags: None,
            nutriments: None, ingredients_text: None, categories: None, image_url: None,
        };
        assert_eq!(classify_sat_fat_density(&p), None);
    }

    #[test]
    fn test_classify_sat_fat_density_zero_kcal() {
        let p = make_product_sat_fat(0.0, 3.0);
        assert_eq!(classify_sat_fat_density(&p), None);
    }

    #[test]
    fn test_analysis_includes_sat_fat_density() {
        let mut p = make_product_sat_fat(100.0, 2.0);
        p.nutriscore_grade = Some("b".into());
        p.nova_group = Some(2);
        let a = analyze(&p);
        assert!(a.sat_fat_density.is_some());
    }
}

/// Salt density classification based on grams of salt per 100 kcal.
/// WHO recommends < 5 g salt per day; higher density means more salt per calorie.
#[derive(Debug, Clone, PartialEq)]
pub enum SaltDensity {
    /// < 0.3 g salt per 100 kcal
    Low,
    /// 0.3-0.8 g salt per 100 kcal
    Moderate,
    /// 0.8-1.5 g salt per 100 kcal
    High,
    /// > 1.5 g salt per 100 kcal (e.g. soy sauce, cured meats, pickles)
    VeryHigh,
}

impl SaltDensity {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::High => "High",
            Self::VeryHigh => "Very high",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Low => "🟢",
            Self::Moderate => "🟡",
            Self::High => "🟠",
            Self::VeryHigh => "🔴",
        }
    }
}

/// Calculate salt density: grams of salt per 100 kcal.
/// Returns None if energy or salt data is missing or energy is near zero.
pub fn classify_salt_density(product: &Product) -> Option<SaltDensity> {
    let n = product.nutriments.as_ref()?;
    let kcal = n.energy_kcal_100g?;
    let salt = n.salt_100g?;
    if kcal < 1.0 {
        return None;
    }
    let per_100 = salt / kcal * 100.0;
    Some(match per_100 {
        x if x < 0.3 => SaltDensity::Low,
        x if x < 0.8 => SaltDensity::Moderate,
        x if x < 1.5 => SaltDensity::High,
        _ => SaltDensity::VeryHigh,
    })
}

#[cfg(test)]
mod salt_density_tests {
    use super::*;
    use crate::api::{Nutriments, Product};

    fn make_product_salt(kcal: f64, salt: f64) -> Product {
        Product {
            code: "1".into(),
            product_name: Some("Test".into()),
            brands: None,
            nutriscore_grade: None,
            nova_group: None,
            additives_tags: None,
            nutriments: Some(Nutriments {
                energy_kcal_100g: Some(kcal),
                salt_100g: Some(salt),
                ..Default::default()
            }),
            ingredients_text: None,
            categories: None,
            image_url: None,
        }
    }

    #[test]
    fn test_classify_salt_density_low() {
        // 0.2g salt per 100kcal => Low
        let p = make_product_salt(100.0, 0.2);
        assert_eq!(classify_salt_density(&p), Some(SaltDensity::Low));
    }

    #[test]
    fn test_classify_salt_density_moderate() {
        // 0.5g salt per 100kcal => Moderate
        let p = make_product_salt(100.0, 0.5);
        assert_eq!(classify_salt_density(&p), Some(SaltDensity::Moderate));
    }

    #[test]
    fn test_classify_salt_density_high() {
        // 1.0g salt per 100kcal => High
        let p = make_product_salt(100.0, 1.0);
        assert_eq!(classify_salt_density(&p), Some(SaltDensity::High));
    }

    #[test]
    fn test_classify_salt_density_very_high() {
        // 2.0g salt per 100kcal => VeryHigh
        let p = make_product_salt(100.0, 2.0);
        assert_eq!(classify_salt_density(&p), Some(SaltDensity::VeryHigh));
    }

    #[test]
    fn test_classify_salt_density_none_without_data() {
        let p = Product {
            code: "1".into(), product_name: None, brands: None,
            nutriscore_grade: None, nova_group: None, additives_tags: None,
            nutriments: None, ingredients_text: None, categories: None, image_url: None,
        };
        assert_eq!(classify_salt_density(&p), None);
    }

    #[test]
    fn test_classify_salt_density_zero_kcal() {
        let p = make_product_salt(0.0, 1.0);
        assert_eq!(classify_salt_density(&p), None);
    }

    #[test]
    fn test_analysis_includes_salt_density() {
        let mut p = make_product_salt(100.0, 0.5);
        p.nutriscore_grade = Some("b".into());
        p.nova_group = Some(2);
        let a = analyze(&p);
        assert!(a.salt_density.is_some());
    }
}
