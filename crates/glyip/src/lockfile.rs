//! Lockfile (Glyip.lock) types and serialisation.
//!
//! The lockfile records the exact versions and sources of every resolved
//! dependency so that builds are reproducible.

use crate::error::{GlyipError, GlyipResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

const LOCKFILE_NAME: &str = "Glyip.lock";

/// The source of a crate — where it was fetched from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "source")]
pub enum CrateSource {
    /// A local filesystem path.
    Path { path: String },
    /// A git repository at a specific revision.
    Git {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        rev: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        branch: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tag: Option<String>,
    },
    /// A registry (index) entry.
    Registry { url: String, checksum: String },
}

/// A single locked dependency entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedCrate {
    /// Crate name.
    pub name: String,
    /// Exact version string.
    pub version: String,
    /// Where the crate came from.
    pub source: CrateSource,
    /// Dependencies of this crate (name → version).
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

/// The full lockfile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lockfile {
    /// Lockfile format version.
    pub version: u32,
    /// Locked crates keyed by `name-version`.
    #[serde(default)]
    pub crates: BTreeMap<String, LockedCrate>,
}

impl Lockfile {
    /// Create a new empty lockfile (format version 1).
    pub fn new() -> Self {
        Self {
            version: 1,
            crates: BTreeMap::new(),
        }
    }

    /// Read a lockfile from the given project directory.
    pub fn read_from_dir(dir: &Path) -> GlyipResult<Self> {
        let path = dir.join(LOCKFILE_NAME);
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = std::fs::read_to_string(&path)?;
        Self::parse(&content)
    }

    /// Parse lockfile content from a string.
    pub fn parse(content: &str) -> GlyipResult<Self> {
        toml::from_str(content).map_err(|e| GlyipError::ConfigParse(e.to_string()))
    }

    /// Write the lockfile to the given project directory.
    pub fn write_to_dir(&self, dir: &Path) -> GlyipResult<()> {
        let path = dir.join(LOCKFILE_NAME);
        let content =
            toml::to_string_pretty(self).map_err(|e| GlyipError::ConfigParse(e.to_string()))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Add a locked crate entry.
    pub fn add_crate(&mut self, krate: LockedCrate) {
        let key = format!("{}-{}", krate.name, krate.version);
        self.crates.insert(key, krate);
    }

    /// Look up a locked crate by name and version.
    pub fn get_crate(&self, name: &str, version: &str) -> Option<&LockedCrate> {
        let key = format!("{}-{}", name, version);
        self.crates.get(&key)
    }

    /// Iterate over all locked crates.
    pub fn crates(&self) -> impl Iterator<Item = &LockedCrate> {
        self.crates.values()
    }

    /// Number of locked crates.
    pub fn len(&self) -> usize {
        self.crates.len()
    }

    /// Whether the lockfile contains no crates.
    pub fn is_empty(&self) -> bool {
        self.crates.is_empty()
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}
