use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::discovery::discover_files;
use crate::facts::{FileFacts, extract_facts};
use crate::graph::{GraphAnalysis, analyze_graph};

pub struct ProjectContext {
    pub root: PathBuf,
    pub effective_extensions: Vec<String>,
    pub scanned_files: Vec<PathBuf>,
    pub facts: Vec<FileFacts>,
    pub graph: GraphAnalysis,
}

impl ProjectContext {
    pub fn analyze(root: &Path, effective_extensions: Vec<String>) -> Result<Self> {
        let scanned_files = discover_files(root, &effective_extensions)?;
        let mut facts = Vec::with_capacity(scanned_files.len());

        for path in &scanned_files {
            let data = fs::read_to_string(path)?;
            if let Some(file_facts) = extract_facts(path, root, &data, &effective_extensions)? {
                facts.push(file_facts);
            }
        }

        let graph = analyze_graph(&facts);

        Ok(Self {
            root: root.to_path_buf(),
            effective_extensions,
            scanned_files,
            facts,
            graph,
        })
    }

    pub fn files_scanned(&self) -> usize {
        self.scanned_files.len()
    }

    pub fn modules(&self) -> usize {
        self.facts.len()
    }
}
