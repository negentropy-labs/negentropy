pub mod json;
pub mod terminal;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub id: String,
    pub severity: Severity,
    pub metric: String,
    pub file: String,
    pub line: usize,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Ok,
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricSummary {
    pub id: String,
    pub score: f64,
    pub severity: Severity,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub total_files: usize,
    pub metrics: Vec<MetricSummary>,
    pub diagnostics: Vec<Diagnostic>,
}
