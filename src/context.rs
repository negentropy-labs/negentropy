use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{Bfs, Reversed};
use tree_sitter::Node;

use crate::lang::ParsedFile;

#[derive(Debug, Clone)]
pub struct ImportFact {
    pub raw_target: String,
    pub resolved_target: Option<String>,
    pub distance: usize,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct ExportFact {
    pub snippet: String,
    pub line: usize,
    pub signature_nodes: usize,
    pub body_nodes: usize,
    pub iie: f64,
}

#[derive(Debug, Clone)]
pub struct EadFact {
    pub entity: String,
    pub line: usize,
    pub self_accesses: usize,
    pub external_accesses: usize,
    pub ead: usize,
}

#[derive(Debug, Clone)]
pub struct MutableDeclaration {
    pub name: String,
    pub line: usize,
    pub mutated: bool,
}

#[derive(Debug, Clone)]
pub struct NewExpressionFact {
    pub constructor: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct MemberWrite {
    pub entity: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct FileFacts {
    pub file: String,
    pub module_key: String,
    pub imports: Vec<ImportFact>,
    pub exports: Vec<ExportFact>,
    pub ead_entities: Vec<EadFact>,
    pub mutable_declarations: Vec<MutableDeclaration>,
    pub constructor_params: usize,
    pub new_expressions: Vec<NewExpressionFact>,
    pub member_writes: Vec<MemberWrite>,
}

#[derive(Debug, Clone)]
pub struct GraphContext {
    pub tce: f64,
    pub tcr_by_module: Vec<(String, f64)>,
    pub cycles: Vec<Vec<String>>,
}

pub struct ProjectContext {
    pub files: Vec<ParsedFile>,
    pub facts: Vec<FileFacts>,
    pub graph: GraphContext,
}

impl ProjectContext {
    pub fn build(files: Vec<ParsedFile>, extensions: &[&str]) -> Self {
        let module_keys = files
            .iter()
            .map(|file| normalize_path(&file.path))
            .collect::<HashSet<_>>();

        let facts = files
            .iter()
            .map(|file| extract_file_facts(file, extensions, &module_keys))
            .collect::<Vec<_>>();

        let graph = build_graph(&facts, extensions);

        Self {
            files,
            facts,
            graph,
        }
    }
}

fn extract_file_facts(
    file: &ParsedFile,
    extensions: &[&str],
    module_keys: &HashSet<String>,
) -> FileFacts {
    let root = file.tree.root_node();

    let imports = collect_imports(file, root, extensions, module_keys);
    let exports = collect_exports(file, root);
    let ead_entities = collect_ead_entities(file, root);
    let (mutable_declarations, member_writes) = collect_state_facts(file, root);
    let constructor_params = count_constructor_params(file, root);
    let new_expressions = collect_new_expressions(file, root);

    FileFacts {
        file: file.path.display().to_string(),
        module_key: normalize_path(&file.path),
        imports,
        exports,
        ead_entities,
        mutable_declarations,
        constructor_params,
        new_expressions,
        member_writes,
    }
}

fn collect_imports(
    file: &ParsedFile,
    root: Node<'_>,
    extensions: &[&str],
    module_keys: &HashSet<String>,
) -> Vec<ImportFact> {
    let mut imports = Vec::new();

    walk_named_nodes(root, &mut |node| {
        let kind = node.kind();
        if kind != "import_statement" && kind != "export_statement" {
            return;
        }

        let Some(raw_target) = extract_string_child(node, &file.source) else {
            return;
        };

        if !raw_target.starts_with("./") && !raw_target.starts_with("../") {
            return;
        }

        imports.push(ImportFact {
            distance: relative_distance(&raw_target),
            resolved_target: resolve_import_target(
                &file.path,
                &raw_target,
                extensions,
                module_keys,
            ),
            line: node.start_position().row + 1,
            raw_target,
        });
    });

    imports
}

fn collect_exports(file: &ParsedFile, root: Node<'_>) -> Vec<ExportFact> {
    let mut exports = Vec::new();

    walk_named_nodes(root, &mut |node| {
        if node.kind() != "export_statement" {
            return;
        }

        let (signature_nodes, body_nodes) = count_surface_vs_body(node);
        if body_nodes == 0 {
            return;
        }

        let snippet = node
            .utf8_text(&file.source)
            .ok()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(60)
            .collect::<String>();

        exports.push(ExportFact {
            line: node.start_position().row + 1,
            iie: signature_nodes as f64 / body_nodes as f64,
            snippet,
            signature_nodes,
            body_nodes,
        });
    });

    exports
}

fn collect_ead_entities(file: &ParsedFile, root: Node<'_>) -> Vec<EadFact> {
    let mut entities = Vec::new();

    let mut root_cursor = root.walk();
    for child in root.named_children(&mut root_cursor) {
        match child.kind() {
            "class_declaration" => {
                let class_name = child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(&file.source).ok())
                    .unwrap_or("?");
                if let Some(body) = child.child_by_field_name("body") {
                    let mut body_cursor = body.walk();
                    for method in body.named_children(&mut body_cursor) {
                        if method.kind() != "method_definition" {
                            continue;
                        }
                        let method_name = method
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(&file.source).ok())
                            .unwrap_or("?");
                        if method_name == "constructor" {
                            continue;
                        }
                        entities.push(build_ead_fact(
                            &format!("{class_name}.{method_name}()"),
                            method,
                            method.start_position().row + 1,
                        ));
                    }
                }
            }
            "function_declaration" => {
                let func_name = child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(&file.source).ok())
                    .unwrap_or("?");
                entities.push(build_ead_fact(
                    &format!("{func_name}()"),
                    child,
                    child.start_position().row + 1,
                ));
            }
            "export_statement" => {
                if let Some(func) = find_child_by_kind(child, "function_declaration") {
                    let func_name = func
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(&file.source).ok())
                        .unwrap_or("?");
                    entities.push(build_ead_fact(
                        &format!("{func_name}()"),
                        func,
                        func.start_position().row + 1,
                    ));
                }
            }
            _ => {}
        }
    }

    entities
}

fn build_ead_fact(entity: &str, node: Node<'_>, line: usize) -> EadFact {
    let (self_accesses, external_accesses) = count_member_accesses(node);
    let ead = external_accesses.saturating_sub(self_accesses);

    EadFact {
        entity: entity.to_string(),
        line,
        self_accesses,
        external_accesses,
        ead,
    }
}

fn collect_state_facts(
    file: &ParsedFile,
    root: Node<'_>,
) -> (Vec<MutableDeclaration>, Vec<MemberWrite>) {
    let mut mutable_names = HashMap::new();
    let mut mutated_names = HashSet::new();
    let mut member_writes = Vec::new();

    walk_named_nodes(root, &mut |node| {
        if is_mutable_declaration(node, &file.source) {
            for (name, line) in declared_identifiers(node, &file.source) {
                mutable_names.insert(name, line);
            }
        }

        if node.kind() == "assignment_expression" {
            if let Some(left) = node.child_by_field_name("left") {
                if left.kind() == "identifier" {
                    if let Ok(name) = left.utf8_text(&file.source) {
                        if mutable_names.contains_key(name) {
                            mutated_names.insert(name.to_string());
                        }
                    }
                } else if left.kind() == "member_expression" {
                    if let Some(entity) = member_entity(left, &file.source) {
                        member_writes.push(MemberWrite {
                            entity,
                            line: left.start_position().row + 1,
                        });
                    }
                }
            }
        }

        if node.kind() == "update_expression" {
            if let Some(argument) = node.child_by_field_name("argument") {
                if argument.kind() == "identifier" {
                    if let Ok(name) = argument.utf8_text(&file.source) {
                        if mutable_names.contains_key(name) {
                            mutated_names.insert(name.to_string());
                        }
                    }
                } else if argument.kind() == "member_expression" {
                    if let Some(entity) = member_entity(argument, &file.source) {
                        member_writes.push(MemberWrite {
                            entity,
                            line: argument.start_position().row + 1,
                        });
                    }
                }
            }
        }
    });

    let mutable_declarations = mutable_names
        .into_iter()
        .map(|(name, line)| MutableDeclaration {
            mutated: mutated_names.contains(&name),
            name,
            line,
        })
        .collect::<Vec<_>>();

    (mutable_declarations, member_writes)
}

fn count_constructor_params(file: &ParsedFile, root: Node<'_>) -> usize {
    let mut total = 0usize;

    walk_named_nodes(root, &mut |node| {
        if node.kind() != "method_definition" {
            return;
        }

        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let Ok(name) = name_node.utf8_text(&file.source) else {
            return;
        };
        if name != "constructor" {
            return;
        }

        let Some(params) = node.child_by_field_name("parameters") else {
            return;
        };

        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            let kind = child.kind();
            if kind == "required_parameter" || kind == "optional_parameter" {
                total += 1;
            }
        }
    });

    total
}

fn collect_new_expressions(file: &ParsedFile, root: Node<'_>) -> Vec<NewExpressionFact> {
    let mut new_exprs = Vec::new();

    walk_named_nodes(root, &mut |node| {
        if node.kind() != "new_expression" {
            return;
        }

        let constructor = node
            .child_by_field_name("constructor")
            .and_then(|n| n.utf8_text(&file.source).ok())
            .unwrap_or("?")
            .to_string();

        new_exprs.push(NewExpressionFact {
            constructor,
            line: node.start_position().row + 1,
        });
    });

    new_exprs
}

fn build_graph(facts: &[FileFacts], extensions: &[&str]) -> GraphContext {
    if facts.is_empty() {
        return GraphContext {
            tce: 0.0,
            tcr_by_module: Vec::new(),
            cycles: Vec::new(),
        };
    }

    let mut graph = DiGraph::<String, ()>::new();
    let mut node_map = HashMap::<String, NodeIndex>::new();
    let mut index_to_file = HashMap::<NodeIndex, String>::new();

    for fact in facts {
        let idx = graph.add_node(fact.file.clone());
        node_map.insert(fact.module_key.clone(), idx);
        index_to_file.insert(idx, fact.file.clone());
    }

    for fact in facts {
        let Some(from_idx) = node_map.get(&fact.module_key).copied() else {
            continue;
        };
        for import in &fact.imports {
            if let Some(ref target) = import.resolved_target {
                let target_key = target.clone();
                if let Some(to_idx) = node_map.get(&target_key).copied() {
                    graph.add_edge(from_idx, to_idx, ());
                } else if let Some(to_idx) = find_target(&target_key, &node_map, extensions) {
                    graph.add_edge(from_idx, to_idx, ());
                }
            }
        }
    }

    let sccs = tarjan_scc(&graph);
    let largest_scc = sccs
        .iter()
        .filter(|scc| scc.len() > 1)
        .map(Vec::len)
        .max()
        .unwrap_or(0);
    let tce = largest_scc as f64 / facts.len() as f64;

    let cycles = sccs
        .iter()
        .filter(|scc| scc.len() > 1)
        .map(|scc| {
            scc.iter()
                .filter_map(|idx| index_to_file.get(idx))
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut tcr_by_module = node_map
        .iter()
        .map(|(module_key, &node_idx)| {
            let reachable = bfs_reverse_reachable_count(&graph, node_idx);
            let score = reachable as f64 / facts.len() as f64;
            let display = facts
                .iter()
                .find(|fact| fact.module_key == *module_key)
                .map(|fact| fact.file.clone())
                .unwrap_or_else(|| module_key.clone());
            (display, score)
        })
        .collect::<Vec<_>>();

    tcr_by_module.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    GraphContext {
        tce,
        tcr_by_module,
        cycles,
    }
}

fn normalize_path(path: &Path) -> String {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    let normalized: PathBuf = components.iter().collect();
    normalized.display().to_string()
}

fn resolve_import_target(
    from_file: &Path,
    import_path: &str,
    extensions: &[&str],
    module_keys: &HashSet<String>,
) -> Option<String> {
    let parent = from_file.parent()?;
    let raw = normalize_path(&parent.join(import_path));

    if module_keys.contains(&raw) {
        return Some(raw);
    }

    for ext in extensions {
        let candidate = format!("{raw}.{}", ext);
        if module_keys.contains(&candidate) {
            return Some(candidate);
        }
        let index_candidate = format!("{raw}/index.{}", ext);
        if module_keys.contains(&index_candidate) {
            return Some(index_candidate);
        }
    }

    None
}

fn find_target(
    base: &str,
    node_map: &HashMap<String, NodeIndex>,
    extensions: &[&str],
) -> Option<NodeIndex> {
    if let Some(idx) = node_map.get(base) {
        return Some(*idx);
    }

    for ext in extensions {
        let candidate = format!("{base}.{}", ext);
        if let Some(idx) = node_map.get(&candidate) {
            return Some(*idx);
        }
        let index_candidate = format!("{base}/index.{}", ext);
        if let Some(idx) = node_map.get(&index_candidate) {
            return Some(*idx);
        }
    }

    None
}

fn bfs_reverse_reachable_count(graph: &DiGraph<String, ()>, start: NodeIndex) -> usize {
    let reversed = Reversed(graph);
    let mut bfs = Bfs::new(&reversed, start);
    let mut count = 0usize;

    while bfs.next(&reversed).is_some() {
        count += 1;
    }

    count.saturating_sub(1)
}

fn walk_named_nodes(node: Node<'_>, visit: &mut dyn FnMut(Node<'_>)) {
    visit(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_named_nodes(child, visit);
    }
}

fn extract_string_child(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "string" {
            return child
                .utf8_text(source)
                .ok()
                .map(|text| text.trim_matches(['"', '\'', '`']).to_string());
        }
    }
    None
}

fn relative_distance(path: &str) -> usize {
    path.split('/').filter(|segment| *segment == "..").count()
}

fn count_surface_vs_body(node: Node<'_>) -> (usize, usize) {
    let mut signature = 0usize;
    let mut body = 0usize;
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
        match current.kind() {
            "formal_parameters"
            | "type_annotation"
            | "type_parameters"
            | "accessibility_modifier"
            | "decorator" => signature += count_descendants(current),
            "statement_block" => body += count_descendants(current),
            _ => {
                let mut cursor = current.walk();
                for child in current.children(&mut cursor) {
                    stack.push(child);
                }
            }
        }
    }

    (signature, body)
}

fn count_descendants(node: Node<'_>) -> usize {
    let mut count = 1usize;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count += count_descendants(child);
    }
    count
}

fn count_member_accesses(node: Node<'_>) -> (usize, usize) {
    let mut self_count = 0usize;
    let mut external_count = 0usize;
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
        if current.kind() == "member_expression" {
            if let Some(object) = current.child_by_field_name("object") {
                if object.kind() == "this" {
                    self_count += 1;
                } else if object.kind() == "identifier" {
                    external_count += 1;
                }
            }
            continue;
        }

        let mut cursor = current.walk();
        for child in current.children(&mut cursor) {
            stack.push(child);
        }
    }

    (self_count, external_count)
}

fn is_mutable_declaration(node: Node<'_>, source: &[u8]) -> bool {
    if node.kind() != "lexical_declaration" {
        return false;
    }

    node.utf8_text(source)
        .ok()
        .map(|text| text.trim_start().starts_with("let "))
        .unwrap_or(false)
}

fn declared_identifiers(node: Node<'_>, source: &[u8]) -> Vec<(String, usize)> {
    let mut names = Vec::new();
    let mut cursor = node.walk();
    for decl in node.named_children(&mut cursor) {
        if decl.kind() != "variable_declarator" {
            continue;
        }
        let Some(name_node) = decl.child_by_field_name("name") else {
            continue;
        };
        if name_node.kind() != "identifier" {
            continue;
        }
        if let Ok(name) = name_node.utf8_text(source) {
            names.push((name.to_string(), name_node.start_position().row + 1));
        }
    }
    names
}

fn member_entity(member: Node<'_>, source: &[u8]) -> Option<String> {
    let object = member.child_by_field_name("object")?;
    let property = member.child_by_field_name("property")?;

    let object_name = if object.kind() == "this" {
        "this".to_string()
    } else {
        object.utf8_text(source).ok()?.to_string()
    };
    let property_name = property.utf8_text(source).ok()?.to_string();

    Some(format!("{object_name}.{property_name}"))
}

fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let result = node
        .children(&mut cursor)
        .find(|child| child.kind() == kind);
    result
}
