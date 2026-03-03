use anyhow::{Result, anyhow};
use tree_sitter::{Node, Parser, Tree};

pub fn parse_source(source: &str) -> Result<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .map_err(|_| anyhow!("failed to initialize tree-sitter javascript language"))?;

    parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("failed to parse source"))
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
