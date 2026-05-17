use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PilotError {
    #[error("protocol parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("file apply error: {0}")]
    Apply(#[from] ApplyError),

    #[error("path security violation: {path} escapes worktree {root}: {reason}")]
    PathEscape { path: String, root: String, reason: String },

    #[error("git operation failed: {0}")]
    Git(String),

    #[error("gate '{gate}' infrastructure failure: {message}")]
    Gate { gate: String, message: String },

    #[error("config error: {0}")]
    Config(String),

    #[error("session error: {0}")]
    Session(String),

    #[error("apply limits exceeded: {0}")]
    Limits(String),

    #[error("io error: {0}")]
    Io(#[source] io::Error),
}

impl PilotError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Parse { .. } => "E0100",
            Self::Apply(e) => e.code(),
            Self::PathEscape { .. } => "E0300",
            Self::Git(_) => "E0400",
            Self::Gate { .. } => "E0500",
            Self::Config(_) => "E0600",
            Self::Session(_) => "E0700",
            Self::Limits(_) => "E0900",
            Self::Io(_) => "E0800",
        }
    }
}

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("FIND text not found in {path}")]
    FindNotFound { path: String },
    #[error("FIND text found {count} times in {path} (expected exactly 1)")]
    FindAmbiguous { path: String, count: usize },
    #[error("file not found: {0}")]
    FileNotFound(String),
    #[error("I/O error during {operation} on {path}: {source}")]
    Io { path: String, operation: String, #[source] source: io::Error },
    #[error("task join failure during {operation}: {reason}")]
    TaskJoin { operation: String, reason: String },
    #[error("apply failed and was rolled back: {detail}")]
    RolledBack { detail: String },
}

impl ApplyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::FindNotFound { .. } => "E0201",
            Self::FindAmbiguous { .. } => "E0202",
            Self::FileNotFound(_) => "E0203",
            Self::Io { .. } => "E0204",
            Self::TaskJoin { .. } => "E0205",
            Self::RolledBack { .. } => "E0206",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_error_codes_documented() {
        let codes = [
            "E0100", "E0201", "E0202", "E0203", "E0204", "E0205", "E0206",
            "E0300", "E0400", "E0500", "E0600", "E0700", "E0800", "E0900",
        ];
        let md = include_str!("../ERROR_CODES.md");
        for code in codes {
            assert!(md.contains(code), "ERROR_CODES.md missing code {code}");
        }
    }

    #[test]
    fn test_task_join_distinct_from_io() {
        let err = ApplyError::TaskJoin { operation: "test".into(), reason: "panic".into() };
        assert_eq!(err.code(), "E0205");
        assert!(!format!("{err}").contains("I/O"));
    }
}
