use std::collections::{HashMap, HashSet};

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::facts::FileFacts;

#[derive(Debug, Clone)]
pub struct GraphAnalysis {
    pub tce: f64,
    pub tcr_by_module: Vec<(String, f64)>,
}

pub fn analyze_graph(facts: &[FileFacts]) -> GraphAnalysis {
    let module_count = facts.len();
    if module_count == 0 {
        return GraphAnalysis {
            tce: 0.0,
            tcr_by_module: Vec::new(),
        };
    }

    let mut graph = DiGraph::<(), ()>::new();
    let mut module_indices = HashMap::new();

    for fact in facts {
        let index = graph.add_node(());
        module_indices.insert(fact.module_id.clone(), index);
    }

    for fact in facts {
        let Some(from) = module_indices.get(&fact.module_id).copied() else {
            continue;
        };

        for import in &fact.imports {
            if let Some(target) = &import.resolved_target
                && let Some(to) = module_indices.get(target).copied()
            {
                graph.add_edge(from, to, ());
            }
        }
    }

    let sccs = kosaraju_scc(&graph);
    let largest_scc = sccs.iter().map(std::vec::Vec::len).max().unwrap_or(0);
    let tce = largest_scc as f64 / module_count as f64;

    let mut reverse_adj = vec![Vec::<usize>::new(); module_count];

    for edge in graph.raw_edges() {
        let from = edge.source().index();
        let to = edge.target().index();
        reverse_adj[to].push(from);
    }

    let index_to_module: HashMap<usize, String> = module_indices
        .iter()
        .map(|(module, index)| (index.index(), module.clone()))
        .collect();

    let mut tcr_by_module = Vec::new();
    for idx in 0..module_count {
        let reachable = reverse_reachable_count(idx, &reverse_adj);
        let score = reachable as f64 / module_count as f64;
        let module = index_to_module
            .get(&idx)
            .cloned()
            .unwrap_or_else(|| format!("module#{idx}"));
        tcr_by_module.push((module, score));
    }

    tcr_by_module.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    GraphAnalysis { tce, tcr_by_module }
}

fn reverse_reachable_count(start: usize, reverse_adj: &[Vec<usize>]) -> usize {
    let mut visited = HashSet::new();
    let mut stack = vec![start];

    while let Some(node) = stack.pop() {
        if !visited.insert(node) {
            continue;
        }
        for parent in &reverse_adj[node] {
            if !visited.contains(parent) {
                stack.push(*parent);
            }
        }
    }

    visited.len()
}

#[allow(dead_code)]
fn _node_idx(n: usize) -> NodeIndex {
    NodeIndex::new(n)
}
