//! Dependency resolution for Glyim projects.
//!
//! Resolves a project's dependency graph from a `Glyip.toml` and optional
//! index, performs cycle detection, and produces a `Lockfile`.

use crate::config::GlyipToml;
use crate::error::{GlyipError, GlyipResult};
use crate::lockfile::{CrateSource, LockedCrate, Lockfile};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// An entry in the crate index — metadata about a published crate.
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// Crate name.
    pub name: String,
    /// Available versions (semver-like strings, newest first).
    pub versions: Vec<String>,
    /// Checksums keyed by version.
    pub checksums: HashMap<String, String>,
}

/// A virtual crate index for dependency resolution.
#[derive(Debug, Clone, Default)]
pub struct CrateIndex {
    entries: HashMap<String, IndexEntry>,
}

impl CrateIndex {
    /// Create a new empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an entry into the index.
    pub fn insert(&mut self, entry: IndexEntry) {
        self.entries.insert(entry.name.clone(), entry);
    }

    /// Look up a crate by name.
    pub fn get(&self, name: &str) -> Option<&IndexEntry> {
        self.entries.get(name)
    }

    /// Resolve the best matching version for a version requirement.
    ///
    /// Currently implements a simple "latest version" strategy. Full semver
    /// matching can be added later.
    pub fn resolve_version(&self, name: &str, version_req: Option<&str>) -> GlyipResult<String> {
        let entry = self
            .entries
            .get(name)
            .ok_or_else(|| GlyipError::DependencyNotFound {
                name: name.to_string(),
                version: version_req.map(String::from),
            })?;

        if entry.versions.is_empty() {
            return Err(GlyipError::DependencyNotFound {
                name: name.to_string(),
                version: version_req.map(String::from),
            });
        }

        // Simple strategy: take the latest version that matches the prefix.
        if let Some(req) = version_req {
            let matching = entry
                .versions
                .iter()
                .find(|v| v.starts_with(req))
                .or_else(|| entry.versions.first())
                .unwrap();
            Ok(matching.clone())
        } else {
            Ok(entry.versions.first().unwrap().clone())
        }
    }
}

/// Resolves the full dependency graph for a project.
#[derive(Debug)]
pub struct DependencyResolver {
    index: CrateIndex,
}

impl DependencyResolver {
    /// Create a new resolver with the given crate index.
    pub fn new(index: CrateIndex) -> Self {
        Self { index }
    }

    /// Create a resolver with an empty index (for path-only dependencies).
    pub fn new_no_index() -> Self {
        Self {
            index: CrateIndex::new(),
        }
    }

    /// Resolve all dependencies from a `Glyip.toml` and produce a `Lockfile`.
    pub fn resolve(&self, config: &GlyipToml, project_dir: &Path) -> GlyipResult<Lockfile> {
        let mut lockfile = Lockfile::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut visit_stack: VecDeque<(String, Option<String>, Option<PathBuf>)> = VecDeque::new();

        // Seed the stack with direct dependencies.
        for (name, dep) in config.all_dependencies() {
            let version = dep.version().map(String::from);
            let path = dep.path().map(PathBuf::from);
            visit_stack.push_back((name.clone(), version, path));
        }

        // Process dependencies breadth-first.
        while let Some((name, version_req, path)) = visit_stack.pop_front() {
            let key = if let Some(ref v) = version_req {
                format!("{}-{}", name, v)
            } else {
                name.clone()
            };

            if visited.contains(&key) {
                continue;
            }
            visited.insert(key.clone());

            let locked = if let Some(p) = path {
                // Path dependency — read the sub-project's Glyip.toml.
                let abs_path = if p.is_absolute() {
                    p.clone()
                } else {
                    project_dir.join(&p)
                };
                self.resolve_path_dep(&name, &abs_path)?
            } else {
                // Index / registry dependency.
                let version = self.index.resolve_version(&name, version_req.as_deref())?;
                let checksum = self
                    .index
                    .get(&name)
                    .and_then(|e| e.checksums.get(&version))
                    .cloned()
                    .unwrap_or_default();
                LockedCrate {
                    name: name.clone(),
                    version,
                    source: CrateSource::Registry {
                        url: "https://index.glyim.dev".to_string(),
                        checksum,
                    },
                    dependencies: BTreeMap::new(),
                }
            };

            lockfile.add_crate(locked);
        }

        // Cycle detection on the resolved graph.
        self.detect_cycles(&lockfile)?;

        Ok(lockfile)
    }

    /// Resolve a path-based dependency.
    fn resolve_path_dep(&self, name: &str, path: &Path) -> GlyipResult<LockedCrate> {
        let config = GlyipToml::read_from_dir(path).unwrap_or_else(|_| GlyipToml {
            package: crate::config::PackageConfig {
                name: name.to_string(),
                version: "0.1.0".to_string(),
                edition: "2024".to_string(),
                authors: Vec::new(),
                description: None,
                bin: None,
                lib: None,
            },
            dependencies: BTreeMap::new(),
            dev_dependencies: BTreeMap::new(),
        });

        Ok(LockedCrate {
            name: name.to_string(),
            version: config.package.version.clone(),
            source: CrateSource::Path {
                path: path.to_string_lossy().to_string(),
            },
            dependencies: BTreeMap::new(),
        })
    }

    /// Detect dependency cycles by checking for back-edges.
    pub fn detect_cycles(&self, lockfile: &Lockfile) -> GlyipResult<()> {
        // Build an adjacency list using owned Strings.
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        for locked in lockfile.crates() {
            let deps: Vec<String> = locked.dependencies.keys().cloned().collect();
            graph.insert(locked.name.clone(), deps);
        }

        // DFS-based cycle detection using owned names.
        let mut white: HashSet<String> = graph.keys().cloned().collect();
        let mut gray: HashSet<String> = HashSet::new();
        let mut black: HashSet<String> = HashSet::new();
        let mut path: Vec<String> = Vec::new();

        let nodes: Vec<String> = graph.keys().cloned().collect();
        for node in nodes {
            if white.contains(&node) {
                Self::dfs_cycle(&node, &graph, &mut white, &mut gray, &mut black, &mut path)?;
            }
        }
        Ok(())
    }

    fn dfs_cycle(
        node: &str,
        graph: &HashMap<String, Vec<String>>,
        white: &mut HashSet<String>,
        gray: &mut HashSet<String>,
        black: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> GlyipResult<()> {
        white.remove(node);
        gray.insert(node.to_string());
        path.push(node.to_string());

        if let Some(neighbors) = graph.get(node) {
            for neighbor in neighbors {
                if gray.contains(neighbor) {
                    let cycle_start = path.iter().position(|n| n == neighbor).unwrap_or(0);
                    let cycle: Vec<String> = path[cycle_start..]
                        .iter()
                        .chain(std::iter::once(neighbor))
                        .cloned()
                        .collect();
                    return Err(GlyipError::DependencyCycle(cycle));
                }
                if white.contains(neighbor) {
                    Self::dfs_cycle(neighbor, graph, white, gray, black, path)?;
                }
            }
        }

        path.pop();
        gray.remove(node);
        black.insert(node.to_string());
        Ok(())
    }
}
