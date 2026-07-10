mod delta;
mod metrics;

use serde::{Deserialize, Serialize};

use crate::model::{ComputedMetrics, Dimension, Hotspot, RiskLevel};
use crate::parser::ParseDiagnostic;

pub use delta::ReportDelta;
pub use metrics::{MetricDefinition, metric_definitions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub files_scanned: usize,
    #[serde(default)]
    pub parsed_files: usize,
    #[serde(default)]
    pub files_with_parse_errors: usize,
    pub modules: usize,
    pub overall_risk: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub tool_version: String,
    pub target_path: String,
    pub effective_extensions: Vec<String>,
    pub summary: Summary,
    #[serde(default)]
    pub parse_diagnostics: Vec<ParseDiagnostic>,
    #[serde(default)]
    pub metric_definitions: Vec<MetricDefinition>,
    pub dimensions: Vec<Dimension>,
    pub hotspots: Vec<Hotspot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<ReportDelta>,
}

impl AnalysisReport {
    pub fn new(
        target_path: String,
        effective_extensions: Vec<String>,
        summary: Summary,
        parse_diagnostics: Vec<ParseDiagnostic>,
        metrics: ComputedMetrics,
    ) -> Self {
        Self {
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            target_path,
            effective_extensions,
            summary,
            parse_diagnostics,
            metric_definitions: metric_definitions(),
            dimensions: metrics.dimensions,
            hotspots: metrics.hotspots,
            delta: None,
        }
    }

    pub fn with_delta(mut self, baseline_path: String, baseline: &AnalysisReport) -> Self {
        self.delta = Some(ReportDelta::between(baseline_path, baseline, &self));
        self
    }
}

pub fn render_table(report: &AnalysisReport) -> String {
    let mut out = String::new();
    out.push_str("Negentropy V2 Report\n");
    out.push_str(&format!("Target: {}\n", report.target_path));
    out.push_str(&format!(
        "Extensions: {}\n",
        report.effective_extensions.join(",")
    ));
    out.push_str(&format!(
        "Files: {}  Parsed: {}  Parse Errors: {}  Modules: {}  Overall Risk: {:?}\n\n",
        report.summary.files_scanned,
        report.summary.parsed_files,
        report.summary.files_with_parse_errors,
        report.summary.modules,
        report.summary.overall_risk
    ));

    out.push_str("Dimensions\n");
    out.push_str("- id | metric | risk | raw\n");
    for dim in &report.dimensions {
        out.push_str(&format!(
            "- {} | {} | {:?} | {}\n",
            dim.id, dim.metric, dim.risk, dim.raw
        ));
    }

    out.push_str("\nMetric Guide\n");
    out.push_str("- id | metric | interpretation\n");
    for definition in &report.metric_definitions {
        out.push_str(&format!(
            "- {} | {} | {} High risk: {}\n",
            definition.id, definition.metric, definition.description, definition.high_risk
        ));
    }

    if !report.parse_diagnostics.is_empty() {
        out.push_str("\nParse Diagnostics\n");
        for diagnostic in &report.parse_diagnostics {
            out.push_str(&format!(
                "- {}:{}:{} [{}] {}\n",
                diagnostic.path,
                diagnostic.line,
                diagnostic.column,
                diagnostic.language,
                diagnostic.message
            ));
        }
    }

    out.push_str("\nHotspots\n");
    for hs in &report.hotspots {
        out.push_str(&format!(
            "- [{}] {} = {} @ {} ({})\n",
            hs.dimension_id, hs.entity, hs.metric_value, hs.location, hs.reason
        ));
    }

    if let Some(delta) = &report.delta {
        out.push_str("\nDelta\n");
        out.push_str(&format!("Baseline: {}\n", delta.baseline_path));
        out.push_str("- id | raw delta | risk delta\n");
        for dimension in &delta.dimensions {
            let raw_delta = dimension
                .raw_delta
                .as_ref()
                .map_or_else(|| "n/a".to_string(), ToString::to_string);
            let risk_delta = dimension
                .risk_delta
                .map_or_else(|| "n/a".to_string(), |value| format!("{value:+}"));
            out.push_str(&format!(
                "- {} | {} | {}\n",
                dimension.id, raw_delta, risk_delta
            ));
        }

        out.push_str(&format!("\nNew Hotspots: {}\n", delta.new_hotspots.len()));
        for hs in &delta.new_hotspots {
            out.push_str(&format!(
                "- [{}] {} = {} @ {} ({})\n",
                hs.dimension_id, hs.entity, hs.metric_value, hs.location, hs.reason
            ));
        }

        out.push_str(&format!(
            "\nResolved Hotspots: {}\n",
            delta.resolved_hotspots.len()
        ));
        for hs in &delta.resolved_hotspots {
            out.push_str(&format!(
                "- [{}] {} = {} @ {} ({})\n",
                hs.dimension_id, hs.entity, hs.metric_value, hs.location, hs.reason
            ));
        }
    }

    out
}
