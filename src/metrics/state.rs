use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::context::ProjectContext;
use crate::model::{Dimension, Hotspot, RiskLevel};

use super::{metric_output, risk_ascending, round3};

pub(super) fn compute(context: &ProjectContext, top_n: usize) -> super::MetricOutput {
    let declared_mutable: usize = context.facts.iter().map(|fact| fact.mutable_declared).sum();
    let mutated_mutable: usize = context.facts.iter().map(|fact| fact.mutable_mutated).sum();
    let sse = declared_mutable as f64 / mutated_mutable.max(1) as f64;
    let sse_risk = risk_ascending(sse, 1.10, 1.60);

    let mut writers: HashMap<String, HashSet<String>> = HashMap::new();
    let mut writer_locations = HashMap::new();
    for file in &context.facts {
        for write in &file.member_writes {
            writers
                .entry(write.entity.clone())
                .or_default()
                .insert(file.module_id.clone());
            writer_locations
                .entry(write.entity.clone())
                .or_insert_with(|| format!("{}:{}", file.module_id, write.line));
        }
    }

    let mut oa_entities = writers
        .into_iter()
        .map(|(entity, writer_set)| {
            let writer_count = writer_set.len();
            let value = if writer_count <= 1 {
                0.0
            } else {
                1.0 - (1.0 / writer_count as f64)
            };
            let location = writer_locations
                .get(&entity)
                .cloned()
                .unwrap_or_else(|| "entity".to_string());
            (entity, value, writer_count, location)
        })
        .collect::<Vec<_>>();

    let oa = if oa_entities.is_empty() {
        0.0
    } else {
        oa_entities
            .iter()
            .map(|(_, value, _, _)| *value)
            .sum::<f64>()
            / oa_entities.len() as f64
    };
    let oa_risk = risk_ascending(oa, 0.10, 0.35);
    let risk = RiskLevel::max(sse_risk, oa_risk);

    let mut hotspots = context
        .facts
        .iter()
        .map(|fact| Hotspot {
            dimension_id: "state_encapsulation".to_string(),
            entity: fact.module_id.clone(),
            metric_value: round3(fact.mutable_declared as f64 / fact.mutable_mutated.max(1) as f64),
            location: "file".to_string(),
            reason: "High mutable declaration expansion (SSE)".to_string(),
        })
        .filter(|hotspot| hotspot.metric_value > 1.0)
        .collect::<Vec<_>>();

    oa_entities.sort_by(|a, b| b.1.total_cmp(&a.1));
    hotspots.extend(
        oa_entities
            .into_iter()
            .filter(|(_, value, _, _)| *value > 0.0)
            .take(top_n)
            .map(|(entity, value, writer_count, location)| Hotspot {
                dimension_id: "state_encapsulation".to_string(),
                entity,
                metric_value: round3(value),
                location,
                reason: format!("Ownership ambiguity across {writer_count} writers (OA)"),
            }),
    );

    hotspots.sort_by(|a, b| b.metric_value.total_cmp(&a.metric_value));
    hotspots.truncate(top_n);

    metric_output(
        Dimension {
            id: "state_encapsulation".to_string(),
            metric: "SSE+OA".to_string(),
            raw: json!({
                "sse": round3(sse),
                "oa": round3(oa),
            }),
            risk,
        },
        hotspots,
    )
}
