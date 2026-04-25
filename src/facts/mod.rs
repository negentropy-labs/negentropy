mod functions;
mod imports;
mod state;

use std::path::Path;

use anyhow::Result;

use crate::parser::parse_source;

#[derive(Debug, Clone)]
pub struct ImportEdge {
    pub raw_target: String,
    pub resolved_target: Option<String>,
    pub distance: usize,
}

#[derive(Debug, Clone)]
pub struct FunctionMetric {
    pub name: String,
    pub line: usize,
    pub ead: f64,
    pub injected_interactions: usize,
    pub hardcoded_interactions: usize,
}

#[derive(Debug, Clone)]
pub struct MemberWrite {
    pub entity: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct FileFacts {
    pub module_id: String,
    pub imports: Vec<ImportEdge>,
    pub export_complexity: f64,
    pub implementation_complexity: f64,
    pub functions: Vec<FunctionMetric>,
    pub mutable_declared: usize,
    pub mutable_mutated: usize,
    pub member_writes: Vec<MemberWrite>,
}

pub fn extract_facts(
    path: &Path,
    root: &Path,
    source: &str,
    extensions: &[String],
) -> Result<Option<FileFacts>> {
    let tree = parse_source(source)?;
    let root_node = tree.root_node();

    let module_id = relative_module_id(path, root);
    let imports = imports::collect_imports(path, root, source, root_node, extensions);
    let function_facts = functions::collect_function_facts(source, root_node);
    let state_facts = state::collect_state_facts(source, root_node);

    Ok(Some(FileFacts {
        module_id,
        imports,
        export_complexity: function_facts.export_complexity,
        implementation_complexity: function_facts.implementation_complexity,
        functions: function_facts.functions,
        mutable_declared: state_facts.mutable_declared,
        mutable_mutated: state_facts.mutable_mutated,
        member_writes: state_facts.member_writes,
    }))
}

fn relative_module_id(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
