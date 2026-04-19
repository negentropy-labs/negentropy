use std::path::PathBuf;

use clap::Parser;

use negentropy::context::ProjectContext;
use negentropy::lang::{self, LanguageSupport, TypeScriptSupport};
use negentropy::metric;
use negentropy::report::{json, terminal, Report, RiskLevel};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "negentropy", about = "Code architecture entropy analyzer")]
struct Cli {
    /// Path to the directory to analyze
    path: PathBuf,

    /// Output format: table, json, or both
    #[arg(long, default_value = "table")]
    format: String,

    /// Write JSON report to file
    #[arg(long)]
    output: Option<PathBuf>,

    /// Show only the top N hotspots/diagnostics
    #[arg(long, default_value = "10")]
    top: usize,

    /// Run only a specific metric by dimension id
    #[arg(long)]
    metric: Option<String>,

    /// Exit with non-zero if overall risk >= this level (none, medium, high)
    #[arg(long, default_value = "none")]
    fail_on: String,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let lang = TypeScriptSupport;
    let files = lang::parse_directory(&cli.path, &lang);

    if files.is_empty() {
        eprintln!("No TypeScript files found in {}", cli.path.display());
        std::process::exit(1);
    }

    let context = ProjectContext::build(files, lang.extensions());
    let all_metrics = metric::build_metrics();

    let metrics: Vec<_> = if let Some(ref filter) = cli.metric {
        all_metrics
            .into_iter()
            .filter(|m| m.id() == filter.as_str())
            .collect()
    } else {
        all_metrics
    };

    if metrics.is_empty() {
        eprintln!("Unknown metric: {}", cli.metric.unwrap_or_default());
        std::process::exit(1);
    }

    let mut dimensions = Vec::new();
    let mut hotspots = Vec::new();
    let mut diagnostics = Vec::new();

    for m in &metrics {
        let result = m.analyze_project(&context, cli.top);
        dimensions.push(result.dimension);
        hotspots.extend(result.hotspots);
        diagnostics.extend(result.diagnostics);
    }

    let overall_risk = dimensions
        .iter()
        .map(|d| d.risk)
        .fold(RiskLevel::Low, RiskLevel::max);

    let report = Report {
        tool_version: VERSION.to_string(),
        total_files: context.files.len(),
        overall_risk,
        dimensions,
        hotspots,
        diagnostics,
    };

    match cli.format.as_str() {
        "json" => json::print_json(&report),
        "both" => {
            terminal::print_terminal(&report, cli.top);
            println!();
            json::print_json(&report);
        }
        _ => terminal::print_terminal(&report, cli.top),
    }

    if let Some(ref path) = cli.output {
        json::write_json(&report, path)?;
        eprintln!("Report written to {}", path.display());
    }

    // --fail-on gate
    let exit_code = match cli.fail_on.as_str() {
        "high" if report.overall_risk == RiskLevel::High => 1,
        "medium" if report.overall_risk.rank() >= RiskLevel::Medium.rank() => 1,
        _ => 0,
    };

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}
