use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Serialize)]
pub struct Dimension {
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

#[derive(Debug, Clone)]
pub struct ComputedMetrics {
    pub dimensions: Vec<Dimension>,
    pub hotspots: Vec<Hotspot>,
    pub overall_risk: RiskLevel,
}
