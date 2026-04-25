mod cli;
mod context;
mod discovery;
mod facts;
mod graph;
mod metrics;
mod model;
mod parser;
mod report;

use std::fs;

use anyhow::{Context, Result};
use clap::Parser as _;

use crate::cli::{Cli, Commands, FailOn, OutputFormat};
use crate::context::ProjectContext;
use crate::metrics::compute_metrics;
use crate::report::{AnalysisReport, render_table};

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Analyze(args) => analyze(args),
    }
}

fn analyze(args: crate::cli::AnalyzeArgs) -> Result<i32> {
    let effective_extensions = args.effective_extensions()?;
    let context = ProjectContext::analyze(&args.path, effective_extensions)?;
    let metrics = compute_metrics(&context, args.top);

    let mut report = AnalysisReport::new(
        context.root.canonicalize()?.display().to_string(),
        context.effective_extensions.clone(),
        context.files_scanned(),
        context.modules(),
        metrics,
    );

    if let Some(baseline_path) = &args.baseline {
        let baseline_json = fs::read_to_string(baseline_path).with_context(|| {
            format!("failed to read baseline report {}", baseline_path.display())
        })?;
        let baseline_report: AnalysisReport =
            serde_json::from_str(&baseline_json).with_context(|| {
                format!(
                    "failed to parse baseline report {}",
                    baseline_path.display()
                )
            })?;
        report = report.with_delta(baseline_path.display().to_string(), &baseline_report);
    }

    if matches!(args.format, OutputFormat::Json | OutputFormat::Both) {
        let json = serde_json::to_string_pretty(&report)?;
        if let Some(path) = &args.output {
            fs::write(path, &json)?;
        }
        println!("{json}");
    }

    if matches!(args.format, OutputFormat::Table | OutputFormat::Both) {
        println!("{}", render_table(&report));
    }

    let should_fail = match args.fail_on {
        FailOn::None => false,
        FailOn::Medium => {
            report.summary.overall_risk.rank() >= crate::model::RiskLevel::Medium.rank()
        }
        FailOn::High => report.summary.overall_risk.rank() >= crate::model::RiskLevel::High.rank(),
    };

    if should_fail { Ok(2) } else { Ok(0) }
}
