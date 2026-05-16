//! Error types for the Glyip build tool.

use std::fmt;
use std::path::PathBuf;

/// Errors that can occur during build tool operations.
#[derive(Debug)]
pub enum GlyipError {
    /// An I/O error occurred.
    Io(std::io::Error),
    /// Configuration file parse error.
    ConfigParse(String),
    /// Configuration validation error.
    ConfigValidation(String),
    /// Dependency cycle detected.
    DependencyCycle(Vec<String>),
    /// Dependency not found.
    DependencyNotFound {
        name: String,
        version: Option<String>,
    },
    /// Build compilation failed.
    BuildFailed(Vec<glyim_diag::GlyimDiagnostic>),
    /// Cache corruption detected.
    CacheCorrupted(String),
    /// Project directory not found.
    ProjectNotFound(PathBuf),
    /// Project already exists at path.
    ProjectAlreadyExists(PathBuf),
    /// Lockfile conflict.
    LockfileConflict(String),
    /// Entry point source file not found.
    NoEntryPoint(PathBuf),
    /// Generic error with message.
    Other(String),
}

impl fmt::Display for GlyipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::ConfigParse(msg) => write!(f, "config parse error: {}", msg),
            Self::ConfigValidation(msg) => write!(f, "config validation error: {}", msg),
            Self::DependencyCycle(cycle) => {
                write!(f, "dependency cycle detected: {}", cycle.join(" -> "))
            }
            Self::DependencyNotFound { name, version } => {
                if let Some(v) = version {
                    write!(f, "dependency not found: {} v{}", name, v)
                } else {
                    write!(f, "dependency not found: {}", name)
                }
            }
            Self::BuildFailed(diags) => {
                write!(f, "build failed with {} error(s)", diags.len())
            }
            Self::CacheCorrupted(msg) => write!(f, "cache corrupted: {}", msg),
            Self::ProjectNotFound(path) => {
                write!(f, "project not found at {}", path.display())
            }
            Self::ProjectAlreadyExists(path) => {
                write!(f, "project already exists at {}", path.display())
            }
            Self::LockfileConflict(msg) => write!(f, "lockfile conflict: {}", msg),
            Self::NoEntryPoint(dir) => {
                write!(f, "no entry point found in {}", dir.display())
            }
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for GlyipError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for GlyipError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Result type for Glyip operations.
pub type GlyipResult<T> = Result<T, GlyipError>;
