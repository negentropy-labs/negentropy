use std::collections::BTreeMap;

use serde_json::json;

use crate::context::ProjectContext;
use crate::facts::LiteralKind;
use crate::model::{Dimension, Hotspot};

use super::{metric_output, positive_hotspots, risk_ascending, round3};

struct LiteralOccurrence {
    module_id: String,
    line: usize,
}

pub(super) fn compute(context: &ProjectContext, top_n: usize) -> super::MetricOutput {
    let mut literals: BTreeMap<String, Vec<LiteralOccurrence>> = BTreeMap::new();
    let mut total_literals = 0usize;

    for file in &context.facts {
        if file.module.is_test || file.module.is_generated_like {
            continue;
        }

        for literal in &file.literals {
            if !is_interesting_literal(literal.kind, &literal.value, literal.parent_kind.as_deref())
            {
                continue;
            }

            total_literals += 1;
            literals
                .entry(literal_key(literal.kind, &literal.value))
                .or_default()
                .push(LiteralOccurrence {
                    module_id: file.module_id.clone(),
                    line: literal.line,
                });
        }
    }

    let duplicate_pressure = literals
        .values()
        .map(|occurrences| occurrences.len().saturating_sub(1))
        .sum::<usize>() as f64
        / total_literals.max(1) as f64;
    let risk = risk_ascending(duplicate_pressure, 0.05, 0.15);

    let hotspots = literals
        .into_iter()
        .filter(|(_, occurrences)| occurrences.len() > 1)
        .map(|(key, occurrences)| {
            let first = occurrences.first().expect("non-empty duplicate literal");
            Hotspot {
                dimension_id: "literal_consolidation".to_string(),
                entity: key,
                metric_value: round3((occurrences.len() - 1) as f64),
                location: format!("{}:{}", first.module_id, first.line),
                reason: format!(
                    "Literal appears {} times; consider naming the domain concept",
                    occurrences.len()
                ),
            }
        })
        .collect();

    metric_output(
        Dimension {
            id: "literal_consolidation".to_string(),
            metric: "LDP".to_string(),
            raw: json!(round3(duplicate_pressure)),
            risk,
        },
        positive_hotspots(hotspots, top_n),
    )
}

fn is_interesting_literal(kind: LiteralKind, value: &str, parent_kind: Option<&str>) -> bool {
    if matches!(parent_kind, Some("import_statement" | "export_statement")) {
        return false;
    }

    match kind {
        LiteralKind::String | LiteralKind::Template => {
            let trimmed = value.trim();
            trimmed.len() >= 3 && !trimmed.starts_with("./") && !trimmed.starts_with("../")
        }
        LiteralKind::Number => !matches!(value, "0" | "1" | "-1"),
        LiteralKind::Boolean | LiteralKind::Regex => false,
    }
}

fn literal_key(kind: LiteralKind, value: &str) -> String {
    let prefix = match kind {
        LiteralKind::String => "string",
        LiteralKind::Number => "number",
        LiteralKind::Template => "template",
        LiteralKind::Boolean => "boolean",
        LiteralKind::Regex => "regex",
    };
    format!("{prefix}:{}", compact(value))
}

fn compact(value: &str) -> String {
    const MAX_LEN: usize = 80;
    let value = value.replace('\n', "\\n");
    if value.chars().count() <= MAX_LEN {
        value
    } else {
        format!("{}...", value.chars().take(MAX_LEN).collect::<String>())
    }
}
