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
        if let Some(v) = n.saturated_fat_100g { println!("    Sat. Fat:  {:.1} g", v); }
        if let Some(v) = n.carbohydrates_100g { println!("    Carbs:     {:.1} g", v); }
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

pub fn print_daily_summary(date: &str, summary: &crate::daily::DailySummary) {
    println!();
    println!("{}", format!("═══ Daily Intake: {} ═══", date).bold().cyan());

    if summary.entries.is_empty() {
        println!("  No products logged for this day.");
        println!("  Use 'nutriscan log <product>' to start tracking.");
        println!();
        return;
    }

    println!("{}", "  Products:".bold());
    for (i, entry) in summary.entries.iter().enumerate() {
        println!(
            "    {}. {} — {:.1} serving(s)",
            i + 1,
            entry.product_name,
            entry.servings
        );
    }

    println!();
    println!("{}", "  Totals (estimated):".bold());
    println!("    Energy:      {:.0} kcal", summary.total_kcal);
    println!("    Fat:         {:.1} g", summary.total_fat);
    println!("    Carbs:       {:.1} g", summary.total_carbs);
    println!("    Protein:     {:.1} g", summary.total_protein);
    println!("    Sugar:       {:.1} g", summary.total_sugar);
    println!("    Salt:        {:.2} g", summary.total_salt);
    println!("    Fiber:       {:.1} g", summary.total_fiber);
    println!("    Sat. Fat:    {:.1} g", summary.total_saturated_fat);

    if summary.entries.len() > 1 {
        if let Some((ref name, kcal)) = summary.top_kcal_entry {
            println!("    🏆 Top:        {} ({:.0} kcal)", name, kcal);
        }
    }

    if let Some((fat_pct, carb_pct, prot_pct)) = summary.macro_percentages() {
        println!();
        println!("{}", "  Macro split (by calories):".bold());
        println!("    Fat:     {:.0}%", fat_pct);
        println!("    Carbs:   {:.0}%", carb_pct);
        println!("    Protein: {:.0}%", prot_pct);
    }

    println!();
    let verdict = summary.verdict();
    let colored_verdict = if verdict.contains("Low") || verdict.contains("High") {
        verdict.yellow()
    } else {
        verdict.green()
    };
    println!("  💡 {}", colored_verdict);
    println!();
}

pub fn print_weekly_summary(from: &str, to: &str, days: &[(String, crate::daily::DailySummary)]) {
    println!();
    println!("{}", format!("═══ Weekly Summary: {} → {} ═══", from, to).bold().cyan());

    if days.is_empty() {
        println!("  No products logged in this period.");
        println!();
        return;
    }

    let mut week_kcal = 0.0_f64;
    let mut week_protein = 0.0_f64;
    let mut week_fat = 0.0_f64;
    let mut week_carbs = 0.0_f64;
    let mut week_sat_fat = 0.0_f64;
    let mut week_sugar = 0.0_f64;
    let mut week_salt = 0.0_f64;
    let mut week_fiber = 0.0_f64;

    for (date, summary) in days {
        let n = summary.entries.len();
        println!(
            "  {} — {:>6.0} kcal  ({} item{})",
            date,
            summary.total_kcal,
            n,
            if n == 1 { "" } else { "s" }
        );
        week_kcal += summary.total_kcal;
        week_protein += summary.total_protein;
        week_fat += summary.total_fat;
        week_carbs += summary.total_carbs;
        week_sat_fat += summary.total_saturated_fat;
        week_sugar += summary.total_sugar;
        week_salt += summary.total_salt;
        week_fiber += summary.total_fiber;
    }

    let logged_days = days.len() as f64;
    println!();
    println!("{}", "  Averages per logged day:".bold());
    println!("    Energy:  {:.0} kcal", week_kcal / logged_days);
    println!("    Protein: {:.1} g", week_protein / logged_days);
    println!("    Fat:     {:.1} g", week_fat / logged_days);
    println!("    Carbs:   {:.1} g", week_carbs / logged_days);
    println!("    Sat Fat: {:.1} g", week_sat_fat / logged_days);
    println!("    Sugar:   {:.1} g", week_sugar / logged_days);
    println!("    Salt:    {:.2} g", week_salt / logged_days);
    println!("    Fiber:   {:.1} g", week_fiber / logged_days);

    // Weekly macro percentages
    let fat_cal = week_fat * 9.0;
    let carb_cal = week_carbs * 4.0;
    let prot_cal = week_protein * 4.0;
    let total_cal = fat_cal + carb_cal + prot_cal;
    if total_cal >= 1.0 {
        println!();
        println!("{}", "  Avg macro split (by calories):".bold());
        println!("    Fat:     {:.0}%", fat_cal / total_cal * 100.0);
        println!("    Carbs:   {:.0}%", carb_cal / total_cal * 100.0);
        println!("    Protein: {:.0}%", prot_cal / total_cal * 100.0);
    }

    // Weekly warnings
    let avg_salt = week_salt / logged_days;
    let avg_sugar = week_sugar / logged_days;
    let avg_sat_fat = week_sat_fat / logged_days;
    let avg_fiber = week_fiber / logged_days;
    let avg_protein = week_protein / logged_days;
    let avg_kcal = week_kcal / logged_days;
    let mut warnings: Vec<&str> = Vec::new();
    if avg_salt > 5.0 { warnings.push("⚠ Avg salt exceeds 5 g/day"); }
    if avg_sugar > 50.0 { warnings.push("⚠ Avg sugar exceeds 50 g/day"); }
    if avg_sat_fat > 20.0 { warnings.push("⚠ Avg saturated fat exceeds 20 g/day"); }
    if avg_fiber < 25.0 && avg_kcal > 500.0 { warnings.push("💡 Avg fiber below 25 g/day"); }
    if avg_protein < 50.0 && avg_kcal > 500.0 { warnings.push("💡 Avg protein below 50 g/day"); }
    if !warnings.is_empty() {
        println!();
        for w in &warnings {
            println!("  {}", w);
        }
    }
    println!();
}
