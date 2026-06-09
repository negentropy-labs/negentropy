use serde_json::json;

use crate::context::ProjectContext;
use crate::model::{Dimension, Hotspot};

use super::{metric_output, positive_hotspots, risk_ascending, round3};

pub(super) fn compute(context: &ProjectContext, top_n: usize) -> super::MetricOutput {
    let mut edge_scores = Vec::new();
    let mut hotspots = Vec::new();

    for file in &context.facts {
        if file.module.is_generated_like {
            continue;
        }

        for import in &file.imports {
            let Some(target) = &import.resolved_target else {
                continue;
            };

            let distance = directory_distance(&file.module_id, target);
            let internal_penalty = if violates_internal_boundary(&file.module_id, target) {
                1.0
            } else {
                0.0
            };
            let depth_penalty =
                depth_delta(&file.module_id, target).saturating_sub(2) as f64 * 0.25;
            let score = distance as f64 / 2.0 + internal_penalty + depth_penalty;

            edge_scores.push(score);
            hotspots.push(Hotspot {
                dimension_id: "directory_alignment".to_string(),
                entity: format!("{} -> {}", file.module_id, target),
                metric_value: round3(score),
                location: file.module_id.clone(),
                reason: "Import edge cuts across distant or private directory structure"
                    .to_string(),
            });
        }
    }

    let dis = if edge_scores.is_empty() {
        0.0
    } else {
        edge_scores.iter().sum::<f64>() / edge_scores.len() as f64
    };
    let risk = risk_ascending(dis, 0.75, 1.50);

    metric_output(
        Dimension {
            id: "directory_alignment".to_string(),
            metric: "DIS".to_string(),
            raw: json!(round3(dis)),
            risk,
        },
        positive_hotspots(hotspots, top_n),
    )
}

fn directory_distance(source: &str, target: &str) -> usize {
    let source_dir = directory_components(source);
    let target_dir = directory_components(target);
    let shared = common_prefix_len(&source_dir, &target_dir);
    source_dir.len() + target_dir.len() - shared * 2
}

fn depth_delta(source: &str, target: &str) -> usize {
    let source_depth = directory_components(source).len();
    let target_depth = directory_components(target).len();
    source_depth.abs_diff(target_depth)
}

fn directory_components(path: &str) -> Vec<&str> {
    let mut parts = path.split('/').collect::<Vec<_>>();
    parts.pop();
    parts
}

fn common_prefix_len<'a>(a: &[&'a str], b: &[&'a str]) -> usize {
    a.iter()
        .zip(b)
        .take_while(|(left, right)| left == right)
        .count()
}

fn violates_internal_boundary(source: &str, target: &str) -> bool {
    let source_parts = source.split('/').collect::<Vec<_>>();
    let target_parts = target.split('/').collect::<Vec<_>>();
    let Some(internal_idx) = target_parts.iter().position(|part| *part == "internal") else {
        return false;
    };

    source_parts.get(..internal_idx) != target_parts.get(..internal_idx)
}
