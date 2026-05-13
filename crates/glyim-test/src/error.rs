use std::path::PathBuf;

#[derive(Debug)]
pub enum TestDiscoveryError {
    RootNotFound(PathBuf),
    ReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidConfig {
        path: PathBuf,
        message: String,
    },
    InvalidAnnotation {
        path: PathBuf,
        line: usize,
        message: String,
    },
}

impl std::fmt::Display for TestDiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootNotFound(p) => write!(f, "test directory does not exist: {:?}", p),
            Self::ReadFailed { path, source } => write!(f, "read {:?}: {}", path, source),
            Self::InvalidConfig { path, message } => {
                write!(f, "invalid config in {:?}: {}", path, message)
            }
            Self::InvalidAnnotation {
                path,
                line,
                message,
            } => {
                write!(
                    f,
                    "invalid annotation in {:?} line {}: {}",
                    path, line, message
                )
            }
        }
    }
}

impl std::error::Error for TestDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFailed { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum FailureReason {
    CompilePassUnexpectedErrors {
        errors: Vec<String>,
    },
    AnnotationParseError {
        line: usize,
        message: String,
    },
    DiagnosticMismatch {
        missing_count: usize,
        unexpected_count: usize,
        wrong_severity_count: usize,
        details: String,
    },
    ErrorPatternNotFound {
        pattern: String,
    },
    UiOutputDiffers {
        diff: String,
    },
    UiNoExpectedFile {
        path: PathBuf,
    },
    TimeoutExceeded {
        timeout_secs: u64,
    },
    CompilationFailed {
        phase: String,
        message: String,
    },
    RunFailed {
        exit_code: Option<i32>,
        expected_exit_code: Option<i32>,
    },
    StdoutMismatch {
        expected: String,
        actual: String,
    },
    StderrMismatch {
        expected: String,
        actual: String,
    },
    RunTimeout {
        timeout_secs: u64,
    },
}

impl std::fmt::Display for FailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CompilePassUnexpectedErrors { errors } => {
                write!(
                    f,
                    "expected compilation to succeed, got {} error(s):\n  {}",
                    errors.len(),
                    errors.join("\n  ")
                )
            }
            Self::AnnotationParseError { line, message } => {
                write!(f, "annotation parse error at line {}: {}", line, message)
            }
            Self::DiagnosticMismatch {
                missing_count,
                unexpected_count,
                wrong_severity_count,
                details,
            } => {
                write!(
                    f,
                    "diagnostic mismatch ({} missing, {} unexpected, {} wrong severity):\n  {}",
                    missing_count, unexpected_count, wrong_severity_count, details
                )
            }
            Self::ErrorPatternNotFound { pattern } => {
                write!(f, "error-pattern '{}' not found", pattern)
            }
            Self::UiOutputDiffers { diff } => write!(f, "output differs:\n{}", diff),
            Self::UiNoExpectedFile { path } => write!(f, "no expected file: {:?}", path),
            Self::TimeoutExceeded { timeout_secs } => {
                write!(f, "exceeded {}s timeout", timeout_secs)
            }
            Self::CompilationFailed { phase, message } => {
                write!(f, "compilation failed at {}: {}", phase, message)
            }
            Self::RunFailed {
                exit_code,
                expected_exit_code,
            } => {
                write!(
                    f,
                    "run failed: exit code {:?}, expected {:?}",
                    exit_code, expected_exit_code
                )
            }
            Self::StdoutMismatch { expected, actual } => {
                write!(
                    f,
                    "stdout mismatch:\n  expected: {:?}\n  actual:   {:?}",
                    expected, actual
                )
            }
            Self::StderrMismatch { expected, actual } => {
                write!(
                    f,
                    "stderr mismatch:\n  expected: {:?}\n  actual:   {:?}",
                    expected, actual
                )
            }
            Self::RunTimeout { timeout_secs } => {
                write!(f, "run exceeded {}s timeout", timeout_secs)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct TimeoutError {
    pub timeout_secs: u64,
}

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "test exceeded {}s timeout", self.timeout_secs)
    }
}
impl std::error::Error for TimeoutError {}

#[derive(Clone, Debug)]
pub struct AssertionFailure {
    pub expected: String,
    pub actual: String,
    pub ty_description: String,
}
