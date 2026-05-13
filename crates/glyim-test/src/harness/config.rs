use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct TestConfig {
    pub mode: TestMode,
    pub revisions: Vec<String>,
    pub revision_compile_flags: HashMap<String, Vec<String>>,
    pub compile_flags: Vec<String>,
    pub error_patterns: Vec<String>,
    pub needs_llvm: bool,
    pub min_version: Option<String>,
    pub ignore: bool,
    pub only_target: Option<String>,
    pub aux_files: Vec<PathBuf>,
    pub timeout_secs: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            mode: TestMode::CompilePass,
            revisions: Vec::new(),
            revision_compile_flags: HashMap::new(),
            compile_flags: Vec::new(),
            error_patterns: Vec::new(),
            needs_llvm: false,
            min_version: None,
            ignore: false,
            only_target: None,
            aux_files: Vec::new(),
            timeout_secs: 60,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TestMode {
    CompilePass,
    CompileFail,
    Ui,
}

impl FromStr for TestMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "compile-pass" => Ok(Self::CompilePass),
            "compile-fail" => Ok(Self::CompileFail),
            "ui" => Ok(Self::Ui),
            other => Err(format!(
                "unknown test-mode: {:?}. Expected: compile-pass, compile-fail, ui",
                other
            )),
        }
    }
}

impl TestMode {
    pub fn from_str_exact(s: &str) -> Result<Self, String> {
        s.parse()
    }
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::CompilePass => "compile-pass",
            Self::CompileFail => "compile-fail",
            Self::Ui => "ui",
        }
    }
}

pub struct ParsedConfig {
    pub config: TestConfig,
    pub has_explicit_mode: bool,
}

pub fn parse_test_config(source: &str) -> Result<ParsedConfig, String> {
    let mut config = TestConfig::default();
    let mut has_explicit_mode = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("//") {
            if trimmed.is_empty() {
                continue;
            }
            break;
        }
        let content = trimmed[2..].trim();

        if let Some(rest) = content.strip_prefix('[') {
            if let Some(bracket_end) = rest.find(']') {
                let rev = &rest[..bracket_end];
                let directive = rest[bracket_end + 1..].trim();
                if let Some(value) = directive.strip_prefix("compile-flags:") {
                    config
                        .revision_compile_flags
                        .entry(rev.to_string())
                        .or_default()
                        .extend(
                            shell_words::split(value.trim())
                                .map_err(|e| format!("invalid compile flags: {}", e))?,
                        );
                }
                continue;
            }
        }

        if let Some(value) = content.strip_prefix("test-mode:") {
            config.mode = value.parse::<TestMode>()?;
            has_explicit_mode = true;
        } else if let Some(value) = content.strip_prefix("revisions:") {
            config.revisions = value.split_whitespace().map(String::from).collect();
        } else if let Some(value) = content.strip_prefix("compile-flags:") {
            config.compile_flags.extend(
                shell_words::split(value.trim())
                    .map_err(|e| format!("invalid compile flags: {}", e))?,
            );
        } else if let Some(value) = content.strip_prefix("error-pattern:") {
            config.error_patterns.push(value.trim().to_string());
        } else if content == "needs-llvm" {
            config.needs_llvm = true;
        } else if let Some(value) = content.strip_prefix("min-version:") {
            config.min_version = Some(value.trim().to_string());
        } else if content == "ignore" {
            config.ignore = true;
        } else if let Some(value) = content.strip_prefix("only-target:") {
            config.only_target = Some(value.trim().to_string());
        } else if let Some(value) = content.strip_prefix("aux-file:") {
            config.aux_files.push(PathBuf::from(value.trim()));
        } else if let Some(value) = content.strip_prefix("timeout:") {
            config.timeout_secs = value.trim().parse().unwrap_or(60);
        }
    }

    Ok(ParsedConfig {
        config,
        has_explicit_mode,
    })
}
