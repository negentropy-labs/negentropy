mod delta;
mod metrics;

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::LiteralPayloadMode;
use crate::model::{ComputedMetrics, Dimension, Hotspot, ImportResolution, RiskLevel};
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalysisFingerprint {
    pub tool_version: String,
    pub target_path: String,
    pub effective_extensions: Vec<String>,
    pub config_digest: String,
    pub file_set_digest: String,
    pub files_count: usize,
}

impl AnalysisFingerprint {
    pub fn current(
        target_path: String,
        effective_extensions: Vec<String>,
        config_digest: String,
        root: &Path,
        scanned_files: &[PathBuf],
    ) -> Self {
        Self {
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            target_path,
            effective_extensions,
            config_digest,
            file_set_digest: file_set_digest(root, scanned_files),
            files_count: scanned_files.len(),
        }
    }

    fn is_missing(&self) -> bool {
        self.tool_version.is_empty()
            || self.target_path.is_empty()
            || self.config_digest.is_empty()
            || self.file_set_digest.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyReport {
    pub literal_payload: LiteralPayloadMode,
    pub contains_literal_payload: bool,
}

impl Default for PrivacyReport {
    fn default() -> Self {
        Self::from_literal_payload_mode(LiteralPayloadMode::Full)
    }
}

impl PrivacyReport {
    pub fn from_literal_payload_mode(literal_payload: LiteralPayloadMode) -> Self {
        Self {
            literal_payload,
            contains_literal_payload: literal_payload == LiteralPayloadMode::Full,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub tool_version: String,
    pub target_path: String,
    pub effective_extensions: Vec<String>,
    #[serde(default)]
    pub analysis_fingerprint: AnalysisFingerprint,
    pub summary: Summary,
    #[serde(default)]
    pub privacy: PrivacyReport,
    #[serde(default)]
    pub import_resolution: ImportResolution,
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
    #[expect(
        clippy::too_many_arguments,
        reason = "report construction mirrors the top-level serialized schema"
    )]
    pub fn new(
        target_path: String,
        effective_extensions: Vec<String>,
        analysis_fingerprint: AnalysisFingerprint,
        summary: Summary,
        privacy: PrivacyReport,
        import_resolution: ImportResolution,
        parse_diagnostics: Vec<ParseDiagnostic>,
        metrics: ComputedMetrics,
    ) -> Self {
        let mut hotspots = metrics.hotspots;
        apply_privacy(&mut hotspots, privacy.literal_payload);

        Self {
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            target_path,
            effective_extensions,
            analysis_fingerprint,
            summary,
            privacy,
            import_resolution,
            parse_diagnostics,
            metric_definitions: metric_definitions(),
            dimensions: metrics.dimensions,
            hotspots,
            delta: None,
        }
    }

    pub fn with_delta(mut self, baseline_path: String, baseline: &AnalysisReport) -> Result<Self> {
        self.ensure_comparable(baseline)?;
        self.delta = Some(ReportDelta::between(baseline_path, baseline, &self));
        Ok(self)
    }

    fn ensure_comparable(&self, baseline: &AnalysisReport) -> Result<()> {
        let current = &self.analysis_fingerprint;
        let baseline = &baseline.analysis_fingerprint;

        if current.is_missing() || baseline.is_missing() {
            bail!(
                "baseline is not comparable: missing analysis_fingerprint; regenerate the baseline with this negentropy version"
            );
        }

        let mut mismatches = Vec::new();
        if current.tool_version != baseline.tool_version {
            mismatches.push("tool_version");
        }
        if current.target_path != baseline.target_path {
            mismatches.push("target_path");
        }
        if current.effective_extensions != baseline.effective_extensions {
            mismatches.push("effective_extensions");
        }
        if current.config_digest != baseline.config_digest {
            mismatches.push("config_digest");
        }
        if current.file_set_digest != baseline.file_set_digest
            || current.files_count != baseline.files_count
        {
            mismatches.push("file_set");
        }

        if !mismatches.is_empty() {
            bail!(
                "baseline is not comparable: {} differ",
                mismatches.join(", ")
            );
        }

        Ok(())
    }
}

fn apply_privacy(hotspots: &mut [Hotspot], literal_payload: LiteralPayloadMode) {
    if literal_payload == LiteralPayloadMode::Full {
        return;
    }

    for hotspot in hotspots
        .iter_mut()
        .filter(|hotspot| hotspot.dimension_id == "literal_consolidation")
    {
        hotspot.entity = match literal_payload {
            LiteralPayloadMode::Full => hotspot.entity.clone(),
            LiteralPayloadMode::Redacted => {
                format!("<redacted-literal:{}>", short_digest(&hotspot.entity))
            }
            LiteralPayloadMode::None => "<literal-redacted>".to_string(),
        };
    }
}

fn short_digest(value: &str) -> String {
    digest_strings([value]).chars().take(12).collect()
}

fn file_set_digest(root: &Path, scanned_files: &[PathBuf]) -> String {
    let mut paths = scanned_files
        .iter()
        .map(|path| {
            path.strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<Vec<_>>();
    paths.sort();
    digest_strings(paths.iter().map(String::as_str))
}

fn digest_strings<'a>(items: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for item in items {
        hasher.update(item.as_bytes());
        hasher.update(b"\0");
    }
    format!("{:x}", hasher.finalize())
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
    out.push_str(&format!(
        "Import Resolution: candidates={} resolved={} unresolved={} graph_status={:?}\n\n",
        report.import_resolution.internal_import_candidates,
        report.import_resolution.resolved,
        report.import_resolution.unresolved,
        report.import_resolution.graph_status
    ));
    out.push_str(&format!(
        "Privacy: literal_payload={:?} contains_literal_payload={}\n\n",
        report.privacy.literal_payload, report.privacy.contains_literal_payload
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
    out.push_str("- id | metric | status | interpretation\n");
    for definition in &report.metric_definitions {
        out.push_str(&format!(
            "- {} | {} | {} | {} High risk: {}\n",
            definition.id,
            definition.metric,
            definition.status,
            definition.description,
            definition.high_risk
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
