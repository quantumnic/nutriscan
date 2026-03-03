use crate::analyzer::{Analysis, AdditiveWarning, MacroBalance, NutriRating, NovaGroup};
use crate::api::Product;
use colored::*;

pub fn print_analysis(a: &Analysis) {
    println!();
    println!("{}", format!("═══ {} ═══", a.product_name).bold().cyan());
    if a.brands != "Unknown" {
        println!("  Brand: {}", a.brands);
    }

    println!(
        "  Nutri-Score: {} {}",
        a.nutri_rating.emoji(),
        colorize_nutri(&a.nutri_rating)
    );
    println!(
        "  NOVA Group:  {} {}",
        a.nova.emoji(),
        colorize_nova(&a.nova)
    );

    if let Some(ref n) = a.product.nutriments {
        println!();
        println!("{}", "  Nutrition per 100g:".bold());
        if let Some(v) = n.energy_kcal_100g { println!("    Energy:    {:.0} kcal", v); }
        if let Some(v) = n.fat_100g { println!("    Fat:       {:.1} g", v); }
        if let Some(v) = n.sugars_100g { println!("    Sugars:    {:.1} g", v); }
        if let Some(v) = n.salt_100g { println!("    Salt:      {:.2} g", v); }
        if let Some(v) = n.proteins_100g { println!("    Proteins:  {:.1} g", v); }
        if let Some(v) = n.fiber_100g { println!("    Fiber:     {:.1} g", v); }
    }

    if !a.warnings.is_empty() {
        println!();
        println!("{}", "  ⚠ Additive Warnings:".bold().yellow());
        for w in &a.warnings {
            println!("    {} — {}", w.code.red(), w.description);
        }
    }

    if !a.allergens.is_empty() {
        println!();
        println!("{}", "  🥜 Potential Allergens:".bold().magenta());
        for allergen in &a.allergens {
            println!("    • {}", allergen);
        }
    }

    match &a.macro_balance {
        MacroBalance::HighIn(macro_name) => {
            println!("  Macro balance: high in {}", macro_name);
        }
        MacroBalance::Balanced => {
            println!("  Macro balance: balanced");
        }
        MacroBalance::Unknown => {}
    }

    if let Some(score) = a.health_score {
        let (emoji, color_label) = match score {
            80..=100 => ("💚", format!("{}/100 — Excellent", score).green()),
            60..=79 => ("💛", format!("{}/100 — Good", score).yellow()),
            40..=59 => ("🧡", format!("{}/100 — Moderate", score).yellow()),
            20..=39 => ("❤️", format!("{}/100 — Poor", score).red()),
            _ => ("🖤", format!("{}/100 — Bad", score).red().bold()),
        };
        println!("  Health Score: {} {}", emoji, color_label);
    }
    println!();
}

pub fn print_warnings(warnings: &[AdditiveWarning], product_name: &str) {
    if warnings.is_empty() {
        println!("{} No known problematic additives in {}.", "✓".green(), product_name);
    } else {
        println!(
            "{} {} problematic additive(s) in {}:",
            "⚠".yellow(),
            warnings.len(),
            product_name
        );
        for w in warnings {
            println!("  {} — {}", w.code.red(), w.description);
        }
    }
}

pub fn print_comparison(a: &Product, b: &Product, diffs: &[(String, String, String)]) {
    let name_a = a.product_name.as_deref().unwrap_or("Product A");
    let name_b = b.product_name.as_deref().unwrap_or("Product B");

    println!();
    println!("{}", "═══ Product Comparison ═══".bold().cyan());
    println!(
        "  {:20} {:>12} {:>12}",
        "Metric".bold(),
        name_a.bold(),
        name_b.bold()
    );
    println!("  {}", "─".repeat(46));

    // Nutri-Score row
    let ga = a.nutriscore_grade.as_deref().unwrap_or("?").to_uppercase();
    let gb = b.nutriscore_grade.as_deref().unwrap_or("?").to_uppercase();
    println!("  {:20} {:>12} {:>12}", "Nutri-Score", ga, gb);

    // NOVA row
    let na = a.nova_group.map(|v| v.to_string()).unwrap_or_else(|| "?".into());
    let nb = b.nova_group.map(|v| v.to_string()).unwrap_or_else(|| "?".into());
    println!("  {:20} {:>12} {:>12}", "NOVA Group", na, nb);

    for (label, va, vb) in diffs {
        println!("  {:20} {:>12} {:>12}", label, va, vb);
    }
    println!();
}

fn colorize_nutri(r: &NutriRating) -> ColoredString {
    match r {
        NutriRating::Excellent => r.label().green(),
        NutriRating::Good => r.label().green(),
        NutriRating::Moderate => r.label().yellow(),
        NutriRating::Poor => r.label().red(),
        NutriRating::Bad => r.label().red().bold(),
        NutriRating::Unknown => r.label().dimmed(),
    }
}

fn colorize_nova(n: &NovaGroup) -> ColoredString {
    match n {
        NovaGroup::Unprocessed => n.label().green(),
        NovaGroup::ProcessedIngredients => n.label().yellow(),
        NovaGroup::Processed => n.label().yellow(),
        NovaGroup::UltraProcessed => n.label().red().bold(),
        NovaGroup::Unknown => n.label().dimmed(),
    }
}

#[allow(dead_code)]
pub fn format_nutri_rating(r: &NutriRating) -> String {
    format!("{} {}", r.emoji(), r.label())
}

#[allow(dead_code)]
pub fn format_nova(n: &NovaGroup) -> String {
    format!("{} {}", n.emoji(), n.label())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::NutriRating;

    #[test]
    fn test_format_nutri_rating() {
        let s = format_nutri_rating(&NutriRating::Excellent);
        assert!(s.contains("Excellent"));
    }

    #[test]
    fn test_format_nova() {
        let s = format_nova(&NovaGroup::UltraProcessed);
        assert!(s.contains("Ultra-processed"));
    }
}
