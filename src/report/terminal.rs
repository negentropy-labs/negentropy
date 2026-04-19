use colored::Colorize;

use super::{Report, RiskLevel};

pub fn print_terminal(report: &Report, top_n: usize) {
    println!(
        "\n{} v{} — {} files analyzed — Overall Risk: {}\n",
        "negentropy".bold().cyan(),
        report.tool_version,
        report.total_files,
        format_risk(report.overall_risk),
    );

    // Print dimension summaries
    println!("{}", "  Dimensions".bold().underline());
    for d in &report.dimensions {
        let risk_str = format_risk(d.risk);
        println!("    {:<28} {:<6} {}  {}", d.id, d.metric, risk_str, d.raw,);
    }

    // Print hotspots
    if !report.hotspots.is_empty() {
        println!("\n  {}", "Hotspots".bold().underline());
        for hs in report.hotspots.iter().take(top_n) {
            println!(
                "    [{}] {} = {:.3} @ {} ({})",
                hs.dimension_id.dimmed(),
                hs.entity.bold(),
                hs.metric_value,
                hs.location,
                hs.reason,
            );
        }
    }

    // Print top diagnostics
    let mut sorted = report.diagnostics.clone();
    sorted.sort_by(|a, b| b.risk.cmp(&a.risk));
    let shown: Vec<_> = sorted.iter().take(top_n).collect();

    if shown.is_empty() {
        println!("\n  {}", "No diagnostics.".green().bold());
        return;
    }

    println!(
        "\n  {} (top {})\n",
        "Diagnostics".bold().underline(),
        top_n.min(sorted.len())
    );

    for d in &shown {
        let risk = format_risk(d.risk);
        println!("  {} {} {}:{}", risk, d.id.bold(), d.file.dimmed(), d.line);
        println!("       {}", d.message);
        if !d.suggestion.is_empty() {
            println!("       {} {}", "fix:".yellow(), d.suggestion);
        }
        println!();
    }
}

fn format_risk(r: RiskLevel) -> colored::ColoredString {
    match r {
        RiskLevel::Low => " LOW  ".on_green().white().bold(),
        RiskLevel::Medium => " MED  ".on_yellow().black().bold(),
        RiskLevel::High => " HIGH ".on_red().white().bold(),
    }
}
