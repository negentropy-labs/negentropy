pub mod json;
pub mod terminal;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    pub fn rank(self) -> u8 {
        match self {
            RiskLevel::Low => 0,
            RiskLevel::Medium => 1,
            RiskLevel::High => 2,
        }
    }

    pub fn max(a: Self, b: Self) -> Self {
        if a.rank() >= b.rank() {
            a
        } else {
            b
        }
    }
}

/// Map a value to a risk level where higher = worse.
pub fn risk_ascending(value: f64, low_max: f64, medium_max: f64) -> RiskLevel {
    if value <= low_max {
        RiskLevel::Low
    } else if value <= medium_max {
        RiskLevel::Medium
    } else {
        RiskLevel::High
    }
}

/// Map a value to a risk level where lower = worse (e.g. EDR).
pub fn risk_descending(value: f64, low_min: f64, medium_min: f64) -> RiskLevel {
    if value >= low_min {
        RiskLevel::Low
    } else if value >= medium_min {
        RiskLevel::Medium
    } else {
        RiskLevel::High
    }
}

pub fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

pub fn percentile(mut values: Vec<f64>, p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let p = p.clamp(0.0, 1.0);
    let idx = ((values.len() - 1) as f64 * p).round() as usize;
    values[idx]
}

pub fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub id: String,
    pub risk: RiskLevel,
    pub metric: String,
    pub file: String,
    pub line: usize,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DimensionSummary {
    pub id: String,
    pub metric: String,
    pub raw: serde_json::Value,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hotspot {
    pub dimension_id: String,
    pub entity: String,
    pub metric_value: f64,
    pub location: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub tool_version: String,
    pub total_files: usize,
    pub overall_risk: RiskLevel,
    pub dimensions: Vec<DimensionSummary>,
    pub hotspots: Vec<Hotspot>,
    pub diagnostics: Vec<Diagnostic>,
}
