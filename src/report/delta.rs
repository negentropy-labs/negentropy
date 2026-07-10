use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::model::{Hotspot, RiskLevel};

use super::AnalysisReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportDelta {
    pub baseline_path: String,
    pub dimensions: Vec<DimensionDelta>,
    pub resolved_hotspots: Vec<Hotspot>,
    pub new_hotspots: Vec<Hotspot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionDelta {
    pub id: String,
    pub metric: String,
    pub baseline_raw: Option<Value>,
    pub current_raw: Value,
    pub raw_delta: Option<Value>,
    pub baseline_risk: Option<RiskLevel>,
    pub current_risk: RiskLevel,
    pub risk_delta: Option<i8>,
}

impl ReportDelta {
    pub fn between(
        baseline_path: String,
        baseline: &AnalysisReport,
        current: &AnalysisReport,
    ) -> Self {
        let baseline_dimensions = baseline
            .dimensions
            .iter()
            .map(|dimension| (dimension.id.as_str(), dimension))
            .collect::<BTreeMap<_, _>>();

        let dimensions = current
            .dimensions
            .iter()
            .map(|current_dimension| {
                let baseline_dimension = baseline_dimensions.get(current_dimension.id.as_str());
                let baseline_raw = baseline_dimension.map(|dimension| dimension.raw.clone());
                let baseline_risk = baseline_dimension.map(|dimension| dimension.risk);

                DimensionDelta {
                    id: current_dimension.id.clone(),
                    metric: current_dimension.metric.clone(),
                    raw_delta: baseline_dimension
                        .and_then(|dimension| raw_delta(&dimension.raw, &current_dimension.raw)),
                    risk_delta: baseline_risk
                        .map(|risk| current_dimension.risk.rank() as i8 - risk.rank() as i8),
                    baseline_raw,
                    current_raw: current_dimension.raw.clone(),
                    baseline_risk,
                    current_risk: current_dimension.risk,
                }
            })
            .collect();

        let baseline_hotspot_keys = hotspot_keys(&baseline.hotspots);
        let current_hotspot_keys = hotspot_keys(&current.hotspots);

        let resolved_hotspots = baseline
            .hotspots
            .iter()
            .filter(|hotspot| !current_hotspot_keys.contains(&hotspot_key(hotspot)))
            .cloned()
            .collect();

        let new_hotspots = current
            .hotspots
            .iter()
            .filter(|hotspot| !baseline_hotspot_keys.contains(&hotspot_key(hotspot)))
            .cloned()
            .collect();

        Self {
            baseline_path,
            dimensions,
            resolved_hotspots,
            new_hotspots,
        }
    }
}

fn raw_delta(baseline: &Value, current: &Value) -> Option<Value> {
    match (baseline, current) {
        (Value::Number(baseline), Value::Number(current)) => {
            Some(json!(round3(current.as_f64()? - baseline.as_f64()?)))
        }
        (Value::Object(baseline), Value::Object(current)) => object_delta(baseline, current),
        _ => None,
    }
}

fn object_delta(baseline: &Map<String, Value>, current: &Map<String, Value>) -> Option<Value> {
    let keys = baseline
        .keys()
        .chain(current.keys())
        .collect::<BTreeSet<_>>();
    let mut deltas = Map::new();

    for key in keys {
        let Some(baseline_value) = baseline.get(key) else {
            continue;
        };
        let Some(current_value) = current.get(key) else {
            continue;
        };
        let Some(delta) = raw_delta(baseline_value, current_value) else {
            continue;
        };
        deltas.insert(key.clone(), delta);
    }

    if deltas.is_empty() {
        None
    } else {
        Some(Value::Object(deltas))
    }
}

fn hotspot_keys(hotspots: &[Hotspot]) -> BTreeSet<(String, String, String, String, String)> {
    hotspots.iter().map(hotspot_key).collect()
}

fn hotspot_key(hotspot: &Hotspot) -> (String, String, String, String, String) {
    (
        hotspot.dimension_id.clone(),
        hotspot.entity.clone(),
        format!("{:.6}", hotspot.metric_value),
        hotspot.location.clone(),
        hotspot.reason.clone(),
    )
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::raw_delta;

    #[test]
    fn object_raw_delta_tracks_numeric_fields() {
        let baseline = json!({ "sse": 1.0, "oa": 0.2 });
        let current = json!({ "sse": 1.25, "oa": 0.1 });

        assert_eq!(
            raw_delta(&baseline, &current),
            Some(json!({ "oa": -0.1, "sse": 0.25 }))
        );
    }
}
