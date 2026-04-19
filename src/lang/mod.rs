mod typescript;

pub use typescript::TypeScriptSupport;

use std::path::PathBuf;

/// Query kinds shared across all metrics.
/// Each language provides tree-sitter query source for these kinds.
#[derive(Debug, Clone, Copy)]
pub enum QueryKind {
    Imports,
    VariableDeclarations,
    ClassDeclarations,
    Exports,
    MemberAccesses,
    Assignments,
    NewExpressions,
    ConstructorParams,
}

/// Language support trait — thin interface, deep implementation.
/// Implement this once per language to add support.
pub trait LanguageSupport {
    fn language(&self) -> tree_sitter::Language;
    fn extensions(&self) -> &[&str];
    fn query_source(&self, kind: QueryKind) -> &str;
}

/// Parsed file — holds AST and source, no extracted facts.
pub struct ParsedFile {
    pub path: PathBuf,
    pub tree: tree_sitter::Tree,
    pub source: Vec<u8>,
}

/// Parse all supported files in a directory.
pub fn parse_directory(dir: &std::path::Path, lang: &dyn LanguageSupport) -> Vec<ParsedFile> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&lang.language())
        .expect("failed to set language");

    let extensions = lang.extensions();
    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "node_modules"
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !extensions.contains(&ext) {
            continue;
        }
        let source = match std::fs::read(path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if let Some(tree) = parser.parse(&source, None) {
            files.push(ParsedFile {
                path: path.to_path_buf(),
                tree,
                source,
            });
        }
    }

    files
}
