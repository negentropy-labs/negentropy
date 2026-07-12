use serde_json::json;

use crate::context::ProjectContext;
use crate::facts::FileFacts;
use crate::model::{Dimension, Hotspot};

use super::{median, metric_output, positive_hotspots, risk_ascending, round3};

pub(super) fn compute(context: &ProjectContext, top_n: usize) -> super::MetricOutput {
    let mut scores = context
        .facts
        .iter()
        .map(|fact| (fact.module_id.clone(), interface_exposure_score(fact)))
        .collect::<Vec<_>>();

    let median_iie = median(scores.iter().map(|(_, value)| *value).collect());
    let risk = risk_ascending(median_iie, 0.35, 0.80);

    let hotspots = positive_hotspots(
        scores
            .drain(..)
            .map(|(entity, value)| Hotspot {
                dimension_id: "module_abstraction".to_string(),
                entity,
                metric_value: round3(value),
                location: "file".to_string(),
                reason: "High interface-to-implementation ratio".to_string(),
            })
            .collect(),
        top_n,
    );

    metric_output(
        Dimension {
            id: "module_abstraction".to_string(),
            metric: "IIE".to_string(),
            raw: json!(round3(median_iie)),
            risk,
        },
        hotspots,
    )
}

fn interface_exposure_score(fact: &FileFacts) -> f64 {
    let score = fact.export_complexity / fact.implementation_complexity.max(1.0);
    if is_barrel_or_facade(fact) {
        0.0
    } else {
        score
    }
}

fn is_barrel_or_facade(fact: &FileFacts) -> bool {
    let file_name = fact
        .module_id
        .rsplit('/')
        .next()
        .unwrap_or(fact.module_id.as_str())
        .to_ascii_lowercase();
    let facade_name = file_name.starts_with("index.")
        || file_name.contains("facade")
        || file_name.contains("barrel");

    facade_name
        && fact.export_complexity > 0.0
        && fact.functions.is_empty()
        && fact.mutable_declared == 0
        && fact.member_writes.is_empty()
}
