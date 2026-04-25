use std::collections::HashSet;

use serde_json::json;

use crate::context::ProjectContext;
use crate::model::{Dimension, Hotspot};

use super::{metric_output, positive_hotspots, risk_ascending, round3};

pub(super) fn compute(context: &ProjectContext, top_n: usize) -> super::MetricOutput {
    let mut distance_sum = 0usize;
    let mut unique_targets = HashSet::new();
    let mut hotspots = Vec::new();

    for file in &context.facts {
        for import in &file.imports {
            distance_sum += import.distance;
            unique_targets.insert(
                import
                    .resolved_target
                    .clone()
                    .unwrap_or_else(|| import.raw_target.clone()),
            );
            hotspots.push(Hotspot {
                dimension_id: "intent_redundancy".to_string(),
                entity: format!("{} -> {}", file.module_id, import.raw_target),
                metric_value: round3(import.distance as f64),
                location: file.module_id.clone(),
                reason: "Deep relative import path".to_string(),
            });
        }
    }

    let plme = distance_sum as f64 / unique_targets.len().max(1) as f64;
    let risk = risk_ascending(plme, 0.40, 1.20);

    metric_output(
        Dimension {
            id: "intent_redundancy".to_string(),
            metric: "PLME".to_string(),
            raw: json!(round3(plme)),
            risk,
        },
        positive_hotspots(hotspots, top_n),
    )
}
