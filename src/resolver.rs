use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use serde_json::Value;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone)]
pub struct ResolvedImport {
    pub is_internal_candidate: bool,
    pub resolved_target: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModuleResolver {
    root: PathBuf,
    extensions: Vec<String>,
    files: HashSet<PathBuf>,
    packages: Vec<PackageConfig>,
    tsconfigs: Vec<TsConfig>,
}

#[derive(Debug, Clone)]
struct PackageConfig {
    name: Option<String>,
    root: PathBuf,
    imports: Vec<Mapping>,
    exports: Vec<Mapping>,
    main: Option<String>,
    types: Option<String>,
}

#[derive(Debug, Clone)]
struct TsConfig {
    root: PathBuf,
    base_url: PathBuf,
    paths: Vec<Mapping>,
}

#[derive(Debug, Clone)]
struct Mapping {
    pattern: String,
    targets: Vec<String>,
}

impl ModuleResolver {
    pub fn analyze(root: &Path, scanned_files: &[PathBuf], extensions: &[String]) -> Result<Self> {
        let root = normalize_path(root);
        let files = scanned_files
            .iter()
            .map(normalize_path)
            .collect::<HashSet<_>>();
        let mut packages = Vec::new();
        let mut tsconfigs = Vec::new();

        for entry in WalkDir::new(&root)
            .into_iter()
            .filter_entry(should_descend)
            .filter_map(std::result::Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            let file_name = path.file_name().and_then(|name| name.to_str());
            match file_name {
                Some("package.json") => {
                    if let Some(package) = read_package_config(path)? {
                        packages.push(package);
                    }
                }
                Some("tsconfig.json") => {
                    if let Some(tsconfig) = read_tsconfig(path)? {
                        tsconfigs.push(tsconfig);
                    }
                }
                _ => {}
            }
        }

        packages.sort_by(|a, b| {
            b.root
                .components()
                .count()
                .cmp(&a.root.components().count())
        });
        tsconfigs.sort_by(|a, b| {
            b.root
                .components()
                .count()
                .cmp(&a.root.components().count())
        });

        Ok(Self {
            root,
            extensions: extensions.to_vec(),
            files,
            packages,
            tsconfigs,
        })
    }

    pub fn resolve(&self, source: &Path, raw: &str) -> ResolvedImport {
        if raw.starts_with("./") || raw.starts_with("../") {
            return ResolvedImport {
                is_internal_candidate: true,
                resolved_target: self.resolve_relative(source, raw),
            };
        }

        if raw.starts_with('#') {
            return ResolvedImport {
                is_internal_candidate: true,
                resolved_target: self.resolve_package_import(source, raw),
            };
        }

        if let Some(target) = self.resolve_tsconfig_path(source, raw) {
            return ResolvedImport {
                is_internal_candidate: true,
                resolved_target: Some(target),
            };
        }

        if self.is_workspace_package_import(raw) {
            return ResolvedImport {
                is_internal_candidate: true,
                resolved_target: self.resolve_workspace_package(raw),
            };
        }

        ResolvedImport {
            is_internal_candidate: false,
            resolved_target: None,
        }
    }

    fn resolve_relative(&self, source: &Path, raw: &str) -> Option<String> {
        let base = normalize_path(source.parent()?.join(raw));
        self.resolve_file_candidate(&base)
    }

    fn resolve_package_import(&self, source: &Path, raw: &str) -> Option<String> {
        let package = self.nearest_package(source)?;
        self.resolve_mappings(&package.imports, raw, &package.root)
    }

    fn resolve_tsconfig_path(&self, source: &Path, raw: &str) -> Option<String> {
        let tsconfig = self.nearest_tsconfig(source)?;
        self.resolve_mappings(&tsconfig.paths, raw, &tsconfig.base_url)
    }

    fn resolve_workspace_package(&self, raw: &str) -> Option<String> {
        let package = self.packages.iter().find(|package| {
            package.name.as_ref().is_some_and(|name| {
                raw == name
                    || raw
                        .strip_prefix(name)
                        .is_some_and(|rest| rest.starts_with('/'))
            })
        })?;

        let name = package.name.as_ref()?;
        let subpath = raw
            .strip_prefix(name)
            .filter(|rest| rest.starts_with('/'))
            .map(|rest| format!(".{rest}"))
            .unwrap_or_else(|| ".".to_string());

        self.resolve_mappings(&package.exports, &subpath, &package.root)
            .or_else(|| {
                if subpath == "." {
                    package
                        .types
                        .as_ref()
                        .or(package.main.as_ref())
                        .and_then(|target| self.resolve_file_candidate(&package.root.join(target)))
                } else {
                    self.resolve_file_candidate(
                        &package.root.join(subpath.trim_start_matches("./")),
                    )
                }
            })
    }

    fn resolve_mappings(
        &self,
        mappings: &[Mapping],
        specifier: &str,
        base: &Path,
    ) -> Option<String> {
        for mapping in mappings {
            let Some(captures) = match_pattern(&mapping.pattern, specifier) else {
                continue;
            };

            for target in &mapping.targets {
                let target = apply_captures(target, &captures);
                if let Some(resolved) = self.resolve_file_candidate(&base.join(target)) {
                    return Some(resolved);
                }
            }
        }

        None
    }

    fn resolve_file_candidate(&self, base: &Path) -> Option<String> {
        for candidate in self.file_candidates(base) {
            let candidate = normalize_path(candidate);
            if self.files.contains(&candidate) {
                return Some(self.module_id(&candidate));
            }
        }

        None
    }

    fn file_candidates(&self, base: &Path) -> Vec<PathBuf> {
        let base = normalize_path(base);
        let mut candidates = Vec::new();

        if base.extension().is_some() {
            candidates.push(base.clone());
            candidates.extend(source_extension_equivalents(&base));
        } else {
            for ext in &self.extensions {
                candidates.push(base.with_extension(ext.trim_start_matches('.')));
            }
            for ext in &self.extensions {
                candidates.push(
                    base.join("index")
                        .with_extension(ext.trim_start_matches('.')),
                );
            }
        }

        candidates
    }

    fn nearest_package(&self, source: &Path) -> Option<&PackageConfig> {
        let source = normalize_path(source);
        self.packages
            .iter()
            .find(|package| source.starts_with(&package.root))
    }

    fn nearest_tsconfig(&self, source: &Path) -> Option<&TsConfig> {
        let source = normalize_path(source);
        self.tsconfigs
            .iter()
            .find(|tsconfig| source.starts_with(&tsconfig.root))
    }

    fn is_workspace_package_import(&self, raw: &str) -> bool {
        self.packages.iter().any(|package| {
            package.name.as_ref().is_some_and(|name| {
                raw == name
                    || raw
                        .strip_prefix(name)
                        .is_some_and(|rest| rest.starts_with('/'))
            })
        })
    }

    fn module_id(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

fn read_package_config(path: &Path) -> Result<Option<PackageConfig>> {
    let data = fs::read_to_string(path)?;
    let json = serde_json::from_str::<Value>(&data)?;
    let Some(root) = path.parent() else {
        return Ok(None);
    };

    Ok(Some(PackageConfig {
        name: json
            .get("name")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        root: normalize_path(root),
        imports: read_mappings(json.get("imports")),
        exports: read_export_mappings(json.get("exports")),
        main: json
            .get("main")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        types: json
            .get("types")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }))
}

fn read_tsconfig(path: &Path) -> Result<Option<TsConfig>> {
    let data = fs::read_to_string(path)?;
    let json = serde_json::from_str::<Value>(&data)?;
    let Some(root) = path.parent() else {
        return Ok(None);
    };
    let root = normalize_path(root);
    let compiler_options = json.get("compilerOptions");
    let base_url = compiler_options
        .and_then(|options| options.get("baseUrl"))
        .and_then(Value::as_str)
        .map_or_else(|| root.clone(), |base| normalize_path(root.join(base)));

    Ok(Some(TsConfig {
        root,
        base_url,
        paths: read_mappings(compiler_options.and_then(|options| options.get("paths"))),
    }))
}

fn read_mappings(value: Option<&Value>) -> Vec<Mapping> {
    let Some(Value::Object(map)) = value else {
        return Vec::new();
    };

    let mut mappings = Vec::new();
    for (pattern, target) in map {
        let targets = mapping_targets(target);
        if !targets.is_empty() {
            mappings.push(Mapping {
                pattern: pattern.clone(),
                targets,
            });
        }
    }

    mappings.sort_by(|a, b| {
        b.pattern
            .len()
            .cmp(&a.pattern.len())
            .then_with(|| a.pattern.cmp(&b.pattern))
    });
    mappings
}

fn read_export_mappings(value: Option<&Value>) -> Vec<Mapping> {
    let Some(value) = value else {
        return Vec::new();
    };

    match value {
        Value::Object(map) if map.keys().any(|key| key.starts_with('.')) => {
            read_mappings(Some(value))
        }
        _ => {
            let targets = mapping_targets(value);
            if targets.is_empty() {
                Vec::new()
            } else {
                vec![Mapping {
                    pattern: ".".to_string(),
                    targets,
                }]
            }
        }
    }
}

fn mapping_targets(value: &Value) -> Vec<String> {
    match value {
        Value::String(target) => vec![target.clone()],
        Value::Array(targets) => targets
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect(),
        Value::Object(map) => ["import", "default", "types"]
            .iter()
            .filter_map(|key| map.get(*key))
            .flat_map(mapping_targets)
            .collect(),
        _ => Vec::new(),
    }
}

fn match_pattern(pattern: &str, specifier: &str) -> Option<Vec<String>> {
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return (pattern == specifier).then(Vec::new);
    };

    if !specifier.starts_with(prefix) || !specifier.ends_with(suffix) {
        return None;
    }

    Some(vec![
        specifier[prefix.len()..specifier.len() - suffix.len()].to_string(),
    ])
}

fn apply_captures(target: &str, captures: &[String]) -> String {
    let mut output = target.to_string();
    for capture in captures {
        output = output.replacen('*', capture, 1);
    }
    output
}

fn source_extension_equivalents(path: &Path) -> Vec<PathBuf> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("js") => vec![path.with_extension("ts"), path.with_extension("tsx")],
        Some("mjs") => vec![path.with_extension("mts")],
        Some("cjs") => vec![path.with_extension("cts")],
        _ => Vec::new(),
    }
}

fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn should_descend(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    !(name == ".git" || name == "node_modules" || name == "dist" || name == "build")
}
