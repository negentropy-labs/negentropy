use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::ProjectConfig;
use crate::discovery::discover_files;
use crate::facts::{FileFacts, extract_facts_from_tree};
use crate::graph::{GraphAnalysis, analyze_graph};
use crate::model::ImportResolution;
use crate::parser::{ParseDiagnostic, parse_source};
use crate::resolver::ModuleResolver;

pub struct ProjectContext {
    pub root: PathBuf,
    pub effective_extensions: Vec<String>,
    pub scanned_files: Vec<PathBuf>,
    pub facts: Vec<FileFacts>,
    pub parse_diagnostics: Vec<ParseDiagnostic>,
    pub import_resolution: ImportResolution,
    pub config: ProjectConfig,
    pub graph: GraphAnalysis,
}

impl ProjectContext {
    pub fn analyze(
        root: &Path,
        effective_extensions: Vec<String>,
        config: ProjectConfig,
    ) -> Result<Self> {
        let root = root.canonicalize()?;
        let scanned_files = discover_files(&root, &effective_extensions, &config.scan)?;
        let resolver = ModuleResolver::analyze(&root, &scanned_files, &effective_extensions)?;
        let mut facts = Vec::with_capacity(scanned_files.len());
        let mut parse_diagnostics = Vec::new();

        for path in &scanned_files {
            let data = fs::read_to_string(path)?;
            let outcome = parse_source(path, &root, &data)?;

            if let Some(diagnostic) = outcome.diagnostic {
                parse_diagnostics.push(diagnostic);
                continue;
            }

            if let Some(tree) = outcome.tree {
                facts.push(extract_facts_from_tree(
                    path,
                    &root,
                    &data,
                    tree.root_node(),
                    &resolver,
                ));
            }
        }

        let import_resolution = import_resolution(&facts);
        let graph = analyze_graph(&facts);

        Ok(Self {
            root,
            effective_extensions,
            scanned_files,
            facts,
            parse_diagnostics,
            import_resolution,
            config,
            graph,
        })
    }

    pub fn files_scanned(&self) -> usize {
        self.scanned_files.len()
    }

    pub fn modules(&self) -> usize {
        self.facts.len()
    }

    pub fn parsed_files(&self) -> usize {
        self.facts.len()
    }

    pub fn files_with_parse_errors(&self) -> usize {
        self.parse_diagnostics.len()
    }
}

fn import_resolution(facts: &[FileFacts]) -> ImportResolution {
    let internal_import_candidates = facts.iter().map(|fact| fact.imports.len()).sum::<usize>();
    let resolved = facts
        .iter()
        .flat_map(|fact| &fact.imports)
        .filter(|import| import.resolved_target.is_some())
        .count();
    let unresolved = internal_import_candidates.saturating_sub(resolved);

    ImportResolution::new(internal_import_candidates, resolved, unresolved)
}
