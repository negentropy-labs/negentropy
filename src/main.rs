use std::path::PathBuf;

use clap::Parser;

use negentropy::lang::{self, TypeScriptSupport};
use negentropy::metric;
use negentropy::report::{Report, json, terminal};

#[derive(Parser)]
#[command(name = "negentropy", about = "Code architecture entropy analyzer")]
struct Cli {
    /// Path to the directory to analyze
    path: PathBuf,

    /// Output as JSON instead of terminal
    #[arg(long)]
    json: bool,

    /// Show only the top N most severe diagnostics
    #[arg(long, default_value = "10")]
    top: usize,

    /// Run only a specific metric (plme, sse, tce, edr, iie, ead, tcr)
    #[arg(long)]
    metric: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let lang = TypeScriptSupport;
    let files = lang::parse_directory(&cli.path, &lang);

    if files.is_empty() {
        eprintln!("No TypeScript files found in {}", cli.path.display());
        std::process::exit(1);
    }

    let all_metrics = metric::build_metrics(&lang);

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

    let mut report = Report {
        total_files: files.len(),
        metrics: Vec::new(),
        diagnostics: Vec::new(),
    };

    for m in &metrics {
        let result = m.analyze_project(&files);
        report.metrics.push(result.summary);
        report.diagnostics.extend(result.diagnostics);
    }

    if cli.json {
        json::print_json(&report);
    } else {
        terminal::print_terminal(&report, cli.top);
    }
}
