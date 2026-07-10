use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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
        if a.rank() >= b.rank() { a } else { b }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimension {
    pub id: String,
    pub metric: String,
    pub raw: serde_json::Value,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotspot {
    pub dimension_id: String,
    pub entity: String,
    pub metric_value: f64,
    pub location: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResolutionStatus {
    #[default]
    Complete,
    Partial,
    Unavailable,
}

impl ResolutionStatus {
    pub fn from_counts(
        internal_import_candidates: usize,
        resolved: usize,
        unresolved: usize,
    ) -> Self {
        if internal_import_candidates > 0 && resolved == 0 {
            Self::Unavailable
        } else if unresolved > 0 {
            Self::Partial
        } else {
            Self::Complete
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportResolution {
    pub internal_import_candidates: usize,
    pub resolved: usize,
    pub unresolved: usize,
    #[serde(default)]
    pub graph_status: ResolutionStatus,
}

impl ImportResolution {
    pub fn new(internal_import_candidates: usize, resolved: usize, unresolved: usize) -> Self {
        Self {
            internal_import_candidates,
            resolved,
            unresolved,
            graph_status: ResolutionStatus::from_counts(
                internal_import_candidates,
                resolved,
                unresolved,
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComputedMetrics {
    pub dimensions: Vec<Dimension>,
    pub hotspots: Vec<Hotspot>,
    pub overall_risk: RiskLevel,
}
