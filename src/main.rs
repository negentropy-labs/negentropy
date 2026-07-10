mod cli;
mod context;
mod discovery;
mod facts;
mod graph;
mod metrics;
mod model;
mod parser;
mod report;
mod resolver;

use std::fs;

use anyhow::{Context, Result};
use clap::Parser as _;

use crate::cli::{Cli, Commands, FailOn, OutputFormat};
use crate::context::ProjectContext;
use crate::metrics::compute_metrics;
use crate::report::{AnalysisFingerprint, AnalysisReport, Summary, render_table};

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
    let target_path = context.root.display().to_string();
    let analysis_fingerprint = AnalysisFingerprint::current(
        target_path.clone(),
        context.effective_extensions.clone(),
        &context.root,
        &context.scanned_files,
    );

    let mut report = AnalysisReport::new(
        target_path,
        context.effective_extensions.clone(),
        analysis_fingerprint,
        Summary {
            files_scanned: context.files_scanned(),
            parsed_files: context.parsed_files(),
            files_with_parse_errors: context.files_with_parse_errors(),
            modules: context.modules(),
            overall_risk: metrics.overall_risk,
        },
        context.import_resolution.clone(),
        context.parse_diagnostics.clone(),
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
        report = report.with_delta(baseline_path.display().to_string(), &baseline_report)?;
    }

    let json = if matches!(args.format, OutputFormat::Json | OutputFormat::Both) {
        let json = serde_json::to_string_pretty(&report)?;
        println!("{json}");
        Some(json)
    } else {
        None
    };

    let table = if matches!(args.format, OutputFormat::Table | OutputFormat::Both) {
        let table = render_table(&report);
        println!("{table}");
        Some(table)
    } else {
        None
    };

    if let Some(path) = &args.output {
        let content = match args.format {
            OutputFormat::Json | OutputFormat::Both => json.as_deref().expect("json rendered"),
            OutputFormat::Table => table.as_deref().expect("table rendered"),
        };
        fs::write(path, content)?;
    }

    if !report.parse_diagnostics.is_empty() {
        return Ok(1);
    }

    let should_fail = if report.delta.is_some() {
        should_fail_on_delta(&report, args.fail_on)
    } else {
        match args.fail_on {
            FailOn::None => false,
            FailOn::Medium => {
                report.summary.overall_risk.rank() >= crate::model::RiskLevel::Medium.rank()
            }
            FailOn::High => {
                report.summary.overall_risk.rank() >= crate::model::RiskLevel::High.rank()
            }
        }
    };

    if should_fail { Ok(2) } else { Ok(0) }
}

fn should_fail_on_delta(report: &AnalysisReport, fail_on: FailOn) -> bool {
    let threshold = match fail_on {
        FailOn::None => return false,
        FailOn::Medium => crate::model::RiskLevel::Medium,
        FailOn::High => crate::model::RiskLevel::High,
    };
    let Some(delta) = &report.delta else {
        return false;
    };

    let risk_upgrade = delta.dimensions.iter().any(|dimension| {
        dimension
            .risk_delta
            .is_some_and(|risk_delta| risk_delta > 0)
            && dimension.current_risk.rank() >= threshold.rank()
    });
    if risk_upgrade {
        return true;
    }

    delta.new_hotspots.iter().any(|hotspot| {
        report
            .dimensions
            .iter()
            .find(|dimension| dimension.id == hotspot.dimension_id)
            .is_some_and(|dimension| dimension.risk.rank() >= threshold.rank())
    })
}
