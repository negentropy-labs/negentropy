use serde_json::json;

use crate::context::ProjectContext;
use crate::model::{Dimension, Hotspot};

use super::{metric_output, percentile, positive_hotspots, risk_ascending, round3};

pub(super) fn compute(context: &ProjectContext, top_n: usize) -> super::MetricOutput {
    let mut function_ead = Vec::new();

    for file in &context.facts {
        for func in &file.functions {
            function_ead.push((
                format!("{}::{}", file.module_id, func.name),
                func.ead,
                format!("{}:{}", file.module_id, func.line),
            ));
        }
    }

    let p90_ead = percentile(
        function_ead.iter().map(|(_, value, _)| *value).collect(),
        0.90,
    );
    let risk = risk_ascending(p90_ead, 1.0, 3.0);

    let hotspots = positive_hotspots(
        function_ead
            .drain(..)
            .map(|(entity, value, location)| Hotspot {
                dimension_id: "logic_cohesion".to_string(),
                entity,
                metric_value: round3(value),
                location,
                reason: "Reads external attributes more than self attributes".to_string(),
            })
            .collect(),
        top_n,
    );

    metric_output(
        Dimension {
            id: "logic_cohesion".to_string(),
            metric: "EAD".to_string(),
            raw: json!(round3(p90_ead)),
            risk,
        },
        hotspots,
    )
}
