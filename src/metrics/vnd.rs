use std::collections::HashSet;

use serde_json::json;

use crate::context::ProjectContext;
use crate::facts::NameKind;
use crate::model::{Dimension, Hotspot};

use super::{metric_output, positive_hotspots, risk_ascending, round3};

pub(super) fn compute(context: &ProjectContext, top_n: usize) -> super::MetricOutput {
    let mut weighted_vague = 0.0f64;
    let mut weighted_total = 0.0f64;
    let mut hotspots = Vec::new();
    let ubiquitous_terms = ubiquitous_terms(context);

    for file in &context.facts {
        if file.module.is_generated_like {
            continue;
        }

        for name in &file.names {
            let tokens = name_tokens(&name.name);
            if tokens.is_empty() {
                continue;
            }

            let weight = name_weight(name.kind, name.exported);
            let vague_count = tokens
                .iter()
                .filter(|token| is_vague_token(token, &ubiquitous_terms))
                .count();
            weighted_total += tokens.len() as f64 * weight;
            weighted_vague += vague_count as f64 * weight;

            if vague_count > 0 {
                hotspots.push(Hotspot {
                    dimension_id: "naming_clarity".to_string(),
                    entity: format!("{:?}:{}", name.kind, name.name),
                    metric_value: round3(vague_count as f64 / tokens.len() as f64 * weight),
                    location: name.line.map_or_else(
                        || file.module_id.clone(),
                        |line| format!("{}:{line}", file.module_id),
                    ),
                    reason: "Vague naming token reduces local architectural intent".to_string(),
                });
            }
        }
    }

    let vnd = weighted_vague / weighted_total.max(1.0);
    let risk = risk_ascending(vnd, 0.10, 0.25);

    metric_output(
        Dimension {
            id: "naming_clarity".to_string(),
            metric: "VND".to_string(),
            raw: json!(round3(vnd)),
            risk,
        },
        positive_hotspots(hotspots, top_n),
    )
}

fn name_weight(kind: NameKind, exported: bool) -> f64 {
    let base = match kind {
        NameKind::Directory => 1.5,
        NameKind::File | NameKind::Module => 1.25,
        NameKind::Function | NameKind::Class | NameKind::Type => 1.0,
        NameKind::Variable | NameKind::Parameter => 0.75,
    };

    if exported { base * 1.5 } else { base }
}

fn name_tokens(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut prev_lowercase = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_lowercase && !current.is_empty() {
                tokens.push(current.to_ascii_lowercase());
                current.clear();
            }
            prev_lowercase = ch.is_ascii_lowercase() || ch.is_ascii_digit();
            current.push(ch);
        } else {
            if !current.is_empty() {
                tokens.push(current.to_ascii_lowercase());
                current.clear();
            }
            prev_lowercase = false;
        }
    }

    if !current.is_empty() {
        tokens.push(current.to_ascii_lowercase());
    }

    tokens
}

fn ubiquitous_terms(context: &ProjectContext) -> HashSet<String> {
    let mut terms = [
        "core", "shared", "service", "services", "data", "types", "type", "config", "index",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect::<HashSet<_>>();

    terms.extend(
        context
            .config
            .language
            .ubiquitous_terms
            .iter()
            .map(|term| term.to_ascii_lowercase()),
    );
    terms
}

fn is_vague_token(token: &str, ubiquitous_terms: &HashSet<String>) -> bool {
    if ubiquitous_terms.contains(token) {
        return false;
    }

    matches!(
        token,
        "util"
            | "utils"
            | "helper"
            | "helpers"
            | "common"
            | "misc"
            | "base"
            | "manager"
            | "handler"
            | "processor"
            | "info"
            | "stuff"
            | "thing"
            | "things"
            | "constants"
    )
}
