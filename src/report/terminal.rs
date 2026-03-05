use colored::Colorize;

use super::{Report, Severity};

pub fn print_terminal(report: &Report, top_n: usize) {
    println!(
        "\n{} {} files analyzed\n",
        "negentropy".bold().cyan(),
        report.total_files
    );

    // Print metric summaries
    println!("{}", "  Metric Scores".bold().underline());
    for m in &report.metrics {
        let severity_str = format_severity(m.severity);
        println!(
            "    {:<8} {:>6.2}  {}",
            m.id.to_uppercase(),
            m.score,
            severity_str
        );
    }

    // Print top diagnostics
    let mut sorted = report.diagnostics.clone();
    sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
    let shown: Vec<_> = sorted.iter().take(top_n).collect();

    if shown.is_empty() {
        println!("\n  {}", "No issues found.".green().bold());
        return;
    }

    println!(
        "\n  {} (showing top {})\n",
        "Diagnostics".bold().underline(),
        top_n.min(sorted.len())
    );

    for d in &shown {
        let sev = format_severity(d.severity);
        println!(
            "  {} {} {}:{}",
            sev,
            d.id.bold(),
            d.file.dimmed(),
            d.line
        );
        println!("       {}", d.message);
        if !d.suggestion.is_empty() {
            println!("       {} {}", "fix:".yellow(), d.suggestion);
        }
        println!();
    }
}

fn format_severity(s: Severity) -> colored::ColoredString {
    match s {
        Severity::Ok => "  OK  ".on_green().white().bold(),
        Severity::Info => " INFO ".on_blue().white().bold(),
        Severity::Warning => " WARN ".on_yellow().black().bold(),
        Severity::Critical => " CRIT ".on_red().white().bold(),
    }
}
