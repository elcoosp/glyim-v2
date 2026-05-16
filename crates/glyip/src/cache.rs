//! Build cache and target directory management.

use crate::error::GlyipResult;
use crate::fingerprint::FingerprintStore;
use std::fs;
use std::path::{Path, PathBuf};

/// Manages the build cache, target directory, and incremental state.
#[derive(Debug)]
pub struct Cache {
    /// Project root directory.
    root: PathBuf,
    /// Target directory (typically `<root>/target`).
    target_dir: PathBuf,
    /// Fingerprint store for incremental compilation.
    fingerprints: FingerprintStore,
}

impl Cache {
    /// Create a cache manager for the given project root.
    ///
    /// Ensures the `target/` directory exists and loads any previously saved
    /// fingerprints.
    pub fn new(root: &Path) -> GlyipResult<Self> {
        let target_dir = root.join("target");
        fs::create_dir_all(&target_dir)?;
        let fingerprints = FingerprintStore::load_from_dir(&target_dir)?;
        Ok(Self {
            root: root.to_path_buf(),
            target_dir,
            fingerprints,
        })
    }

    /// Return the target directory path.
    pub fn target_dir(&self) -> &Path {
        &self.target_dir
    }

    /// Return the debug output directory.
    pub fn debug_dir(&self) -> PathBuf {
        self.target_dir.join("debug")
    }

    /// Return the release output directory.
    pub fn release_dir(&self) -> PathBuf {
        self.target_dir.join("release")
    }

    /// Return the output directory for the given profile.
    pub fn output_dir(&self, release: bool) -> PathBuf {
        if release {
            self.release_dir()
        } else {
            self.debug_dir()
        }
    }

    /// Return the dependency cache directory.
    pub fn dep_dir(&self) -> PathBuf {
        self.target_dir.join("debug").join("dep")
    }

    /// Return the global cache directory (`~/.glyip/cache`).
    pub fn global_cache_dir() -> PathBuf {
        home_dir().join(".glyip").join("cache")
    }

    /// Check whether a single source file needs recompilation.
    pub fn needs_recompile(&self, source_path: &Path) -> GlyipResult<bool> {
        self.fingerprints.has_changed(source_path)
    }

    /// Check whether any `.g` source file under `src/` has changed.
    pub fn needs_rebuild(&self) -> GlyipResult<bool> {
        let src_dir = self.root.join("src");
        if !src_dir.exists() {
            return Ok(true);
        }
        self.fingerprints.has_any_changed(&src_dir, "g")
    }

    /// Update all fingerprints after a successful build and persist them.
    pub fn mark_built(&mut self) -> GlyipResult<()> {
        let src_dir = self.root.join("src");
        if src_dir.exists() {
            self.fingerprints.update_all(&src_dir, "g")?;
        }
        // Also fingerprint the Glyip.toml
        let config_path = self.root.join("Glyip.toml");
        if config_path.exists() {
            self.fingerprints.update(&config_path)?;
        }
        self.fingerprints.save_to_dir(&self.target_dir)
    }

    /// Remove all build artifacts.
    pub fn clean(&self) -> GlyipResult<()> {
        if self.target_dir.exists() {
            fs::remove_dir_all(&self.target_dir)?;
        }
        Ok(())
    }

    /// Return the expected path of the output binary.
    pub fn output_binary(&self, name: &str, release: bool) -> PathBuf {
        self.output_dir(release).join(name)
    }

    /// Store a compiled artifact in the dependency cache.
    pub fn store_artifact(&self, key: &str, data: &[u8]) -> GlyipResult<PathBuf> {
        let cache_dir = self.dep_dir();
        fs::create_dir_all(&cache_dir)?;
        let path = cache_dir.join(format!("{}.gbc", key));
        fs::write(&path, data)?;
        Ok(path)
    }

    /// Retrieve a compiled artifact from the dependency cache.
    pub fn get_artifact(&self, key: &str) -> GlyipResult<Option<Vec<u8>>> {
        let path = self.dep_dir().join(format!("{}.gbc", key));
        if path.exists() {
            Ok(Some(fs::read(&path)?))
        } else {
            Ok(None)
        }
    }
}

/// Best-effort home directory resolution.
fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}
