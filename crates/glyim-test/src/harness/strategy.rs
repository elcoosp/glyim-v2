use super::compiler::CompileOutput;
use crate::annotations::Annotation;
use crate::comparison::{self, DiagSeverityExt, NormalizedDiag};
use crate::error::FailureReason;
use glyim_diag::GlyimDiagnostic;
use std::path::Path;

pub struct CompilePassStrategy;

impl CompilePassStrategy {
    pub fn evaluate(
        &self,
        diagnostics: &[GlyimDiagnostic],
        source: &str,
    ) -> super::executor::TestOutcome {
        let errors: Vec<String> = diagnostics.iter()
            .filter(|d| d.is_error())
            .map(|e| e.message.clone())
            .collect();
        if errors.is_empty() {
            super::executor::TestOutcome::Passed
        } else {
            super::executor::TestOutcome::Failed {
                reason: FailureReason::CompilePassUnexpectedErrors { errors },
            }
        }
    }
}

pub struct CompileFailStrategy;

impl CompileFailStrategy {
    pub fn evaluate(
        &self,
        diagnostics: &[GlyimDiagnostic],
        source: &str,
        error_patterns: &[String],
    ) -> super::executor::TestOutcome {
        let annotations = match Annotation::parse_all(source) {
            Ok(a) => a,
            Err(e) => {
                return super::executor::TestOutcome::Failed {
                    reason: FailureReason::AnnotationParseError { line: 0, message: e },
                };
            }
        };

        let normalized: Vec<NormalizedDiag> = diagnostics.iter()
            .map(|d| NormalizedDiag::from_glyim_diag(d, source))
            .collect();

        let result = comparison::compare_diagnostics(&annotations, &normalized);

        if result.passed() {
            for pattern in error_patterns {
                if !diagnostics.iter().any(|d| d.message.contains(pattern)) {
                    return super::executor::TestOutcome::Failed {
                        reason: FailureReason::ErrorPatternNotFound { pattern: pattern.clone() },
                    };
                }
            }
            super::executor::TestOutcome::Passed
        } else {
            super::executor::TestOutcome::Failed {
                reason: FailureReason::DiagnosticMismatch {
                    missing_count: result.missing.len(),
                    unexpected_count: result.unexpected.len(),
                    wrong_severity_count: result.wrong_severity.len(),
                    details: format_mismatch(&result),
                },
            }
        }
    }
}

pub struct UiTestStrategy;

impl UiTestStrategy {
    pub fn evaluate(
        &self,
        output: &CompileOutput,
        source: &str,
        test_path: &Path,
        bless: bool,
    ) -> super::executor::TestOutcome {
        let mut text = String::new();

        if let Some(ref tree) = output.syntax_tree {
            text.push_str("=== CST ===\n");
            text.push_str(&format!("{:#?}\n", tree));
        }

        if let Some(ref dm) = output.def_map {
            text.push_str("=== DefMap ===\n");
            text.push_str(&crate::snapshot::format::format_def_map(dm));
        }

        if let Some(ref tc) = output.typeck_result {
            text.push_str("=== Typeck ===\n");
            text.push_str(&format!("{:#?}\n", tc));
        }

        if !output.mir_bodies.is_empty() {
            text.push_str("=== MIR ===\n");
        }

        text.push_str("=== Diagnostics ===\n");
        for diag in &output.diagnostics {
            text.push_str(&format!(
                "{}[{}]: {}\n",
                diag.severity.display_name(), diag.code, diag.message,
            ));
        }

        let normalized = crate::comparison::normalize::normalize_output(
            &text, test_path, &Default::default(),
        );

        let expected_path = test_path.with_extension("expected");

        if bless {
            let _ = std::fs::write(&expected_path, &normalized);
            return super::executor::TestOutcome::Passed;
        }

        if !expected_path.exists() {
            return super::executor::TestOutcome::Failed {
                reason: FailureReason::UiNoExpectedFile { path: expected_path },
            };
        }

        let expected = std::fs::read_to_string(&expected_path).unwrap();
        if normalized == expected {
            super::executor::TestOutcome::Passed
        } else {
            let diff = similar::TextDiff::from_lines(&expected, &normalized);
            let mut diff_str = String::new();
            for change in diff.iter_all_changes() {
                let sign = match change.tag() {
                    similar::ChangeTag::Delete => "-",
                    similar::ChangeTag::Insert => "+",
                    similar::ChangeTag::Equal => " ",
                };
                diff_str.push_str(&format!("{}{}", sign, change));
            }
            super::executor::TestOutcome::Failed {
                reason: FailureReason::UiOutputDiffers { diff: diff_str },
            }
        }
    }
}

fn format_mismatch(result: &comparison::ComparisonResult) -> String {
    let mut reasons = Vec::new();
    for m in &result.missing {
        let line: usize = m.target_line() + 1;
        reasons.push(format!(
            "line {}: expected {} {}",
            line,
            m.severity.display_name(),
            m.pattern.description()
        ));
    }
    for u in &result.unexpected {
        reasons.push(format!(
            "line {}: unexpected {} : {}",
            u.line + 1, u.severity.display_name(), u.message
        ));
    }
    for w in &result.wrong_severity {
        reasons.push(format!(
            "line {}: expected {} got {} : {}",
            w.diagnostic.line + 1,
            w.expected.display_name(),
            w.actual.display_name(),
            w.diagnostic.message
        ));
    }
    reasons.join("\n  ")
}
