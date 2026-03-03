mod cli;
mod discovery;
mod facts;
mod graph;
mod metrics;
mod model;
mod parser;
mod report;

use std::fs;

use anyhow::Result;
use clap::Parser as _;

use crate::cli::{Cli, Commands, FailOn, OutputFormat};
use crate::discovery::discover_files;
use crate::facts::extract_facts;
use crate::graph::analyze_graph;
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
    let files = discover_files(&args.path, &effective_extensions)?;

    let mut facts = Vec::with_capacity(files.len());
    for path in &files {
        let data = fs::read_to_string(path)?;
        if let Some(file_facts) = extract_facts(path, &args.path, &data, &effective_extensions)? {
            facts.push(file_facts);
        }
    }

    let graph = analyze_graph(&facts);
    let metrics = compute_metrics(&facts, &graph, args.top);

    let report = AnalysisReport::new(
        args.path.canonicalize()?.display().to_string(),
        effective_extensions,
        files.len(),
        facts.len(),
        metrics,
    );

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
