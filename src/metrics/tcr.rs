use serde_json::json;

use crate::context::ProjectContext;
use crate::model::{Dimension, Hotspot};

use super::{metric_output, positive_hotspots, risk_ascending, round3};

pub(super) fn compute(context: &ProjectContext, top_n: usize) -> super::MetricOutput {
    let max_tcr = context
        .graph
        .tcr_by_module
        .first()
        .map(|(_, value)| *value)
        .unwrap_or(0.0);
    let risk = risk_ascending(max_tcr, 0.10, 0.30);

    let hotspots = positive_hotspots(
        context
            .graph
            .tcr_by_module
            .iter()
            .map(|(entity, value)| Hotspot {
                dimension_id: "change_blast_radius".to_string(),
                entity: entity.clone(),
                metric_value: round3(*value),
                location: "module".to_string(),
                reason: "Large reverse transitive impact radius".to_string(),
            })
            .collect(),
        top_n,
    );

    metric_output(
        Dimension {
            id: "change_blast_radius".to_string(),
            metric: "TCR".to_string(),
            raw: json!(round3(max_tcr)),
            risk,
        },
        hotspots,
    )
}
