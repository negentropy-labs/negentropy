use std::path::Path;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser, Tree};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseDiagnostic {
    pub path: String,
    pub language: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

pub struct ParseOutcome {
    pub tree: Option<Tree>,
    pub diagnostic: Option<ParseDiagnostic>,
}

#[derive(Debug, Clone, Copy)]
enum SourceLanguage {
    JavaScript,
    TypeScript,
    Tsx,
}

impl SourceLanguage {
    fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("ts" | "mts" | "cts") => Self::TypeScript,
            Some("tsx") => Self::Tsx,
            _ => Self::JavaScript,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
        }
    }

    fn set_parser_language(self, parser: &mut Parser) -> Result<()> {
        match self {
            Self::JavaScript => parser
                .set_language(&tree_sitter_javascript::LANGUAGE.into())
                .map_err(|_| anyhow!("failed to initialize tree-sitter javascript language")),
            Self::TypeScript => parser
                .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
                .map_err(|_| anyhow!("failed to initialize tree-sitter typescript language")),
            Self::Tsx => parser
                .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
                .map_err(|_| anyhow!("failed to initialize tree-sitter tsx language")),
        }
    }
}

pub fn parse_source(path: &Path, root: &Path, source: &str) -> Result<ParseOutcome> {
    let language = SourceLanguage::from_path(path);
    let mut parser = Parser::new();
    language.set_parser_language(&mut parser)?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("failed to parse source"))?;

    let root_node = tree.root_node();
    if root_node.has_error() {
        return Ok(ParseOutcome {
            tree: None,
            diagnostic: Some(parse_diagnostic(path, root, language, root_node)),
        });
    }

    Ok(ParseOutcome {
        tree: Some(tree),
        diagnostic: None,
    })
}

pub fn node_text<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    node.utf8_text(source.as_bytes()).ok()
}

pub fn walk_named_nodes(root: Node<'_>, mut visit: impl FnMut(Node<'_>)) {
    fn inner(node: Node<'_>, visit: &mut dyn FnMut(Node<'_>)) {
        visit(node);
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            inner(child, visit);
        }
    }

    inner(root, &mut visit);
}

fn parse_diagnostic(
    path: &Path,
    root: &Path,
    language: SourceLanguage,
    root_node: Node<'_>,
) -> ParseDiagnostic {
    let error_node = first_error_node(root_node).unwrap_or(root_node);
    let position = error_node.start_position();

    ParseDiagnostic {
        path: relative_path(path, root),
        language: language.name().to_string(),
        line: position.row + 1,
        column: position.column + 1,
        message: format!("parse error near {}", error_node.kind()),
    }
}

fn first_error_node(node: Node<'_>) -> Option<Node<'_>> {
    if node.is_error() || node.is_missing() || node.kind() == "ERROR" {
        return Some(node);
    }

    if !node.has_error() {
        return None;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(error) = first_error_node(child) {
            return Some(error);
        }
    }

    None
}

fn relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
