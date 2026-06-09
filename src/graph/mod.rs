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
    let largest_nontrivial_scc = sccs
        .iter()
        .filter(|scc| scc.len() > 1)
        .map(std::vec::Vec::len)
        .max()
        .unwrap_or(0);
    let tce = largest_nontrivial_scc as f64 / module_count as f64;

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
        let impacted_modules = reverse_reachable_count_excluding_self(idx, &reverse_adj);
        let score = if module_count <= 1 {
            0.0
        } else {
            impacted_modules as f64 / (module_count - 1) as f64
        };
        let module = index_to_module
            .get(&idx)
            .cloned()
            .unwrap_or_else(|| format!("module#{idx}"));
        tcr_by_module.push((module, score));
    }

    tcr_by_module.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    GraphAnalysis { tce, tcr_by_module }
}

fn reverse_reachable_count_excluding_self(start: usize, reverse_adj: &[Vec<usize>]) -> usize {
    let mut visited = HashSet::new();
    let mut stack = reverse_adj[start].clone();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{FileFacts, FunctionFact, ImportEdge, MemberWrite};

    fn fact(module_id: &str, imports: &[&str]) -> FileFacts {
        FileFacts {
            module_id: module_id.to_string(),
            module: Default::default(),
            names: Vec::new(),
            literals: Vec::new(),
            imports: imports
                .iter()
                .map(|target| ImportEdge {
                    raw_target: target.to_string(),
                    resolved_target: Some((*target).to_string()),
                    distance: 0,
                })
                .collect(),
            export_complexity: 0.0,
            implementation_complexity: 1.0,
            functions: Vec::<FunctionFact>::new(),
            calls: Vec::new(),
            mutable_declared: 0,
            mutable_mutated: 0,
            member_writes: Vec::<MemberWrite>::new(),
        }
    }

    #[test]
    fn single_module_has_zero_graph_risk() {
        let graph = analyze_graph(&[fact("only.ts", &[])]);

        assert_eq!(graph.tce, 0.0);
        assert_eq!(graph.tcr_by_module, vec![("only.ts".to_string(), 0.0)]);
    }

    #[test]
    fn tcr_excludes_the_target_module_itself() {
        let graph = analyze_graph(&[
            fact("core.ts", &[]),
            fact("a.ts", &["core.ts"]),
            fact("b.ts", &["core.ts"]),
        ]);

        let core = graph
            .tcr_by_module
            .iter()
            .find(|(module, _)| module == "core.ts")
            .expect("core score exists");
        assert_eq!(core.1, 1.0);

        let leaf = graph
            .tcr_by_module
            .iter()
            .find(|(module, _)| module == "a.ts")
            .expect("leaf score exists");
        assert_eq!(leaf.1, 0.0);
    }

    #[test]
    fn tce_counts_only_nontrivial_cycles() {
        let graph = analyze_graph(&[
            fact("a.ts", &["b.ts"]),
            fact("b.ts", &["a.ts"]),
            fact("c.ts", &[]),
        ]);

        assert_eq!(graph.tce, 2.0 / 3.0);
    }
}
