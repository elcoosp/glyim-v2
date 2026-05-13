pub mod normalize;

use crate::annotations::Annotation;
use glyim_diag::{DiagSeverity, GlyimDiagnostic};

pub trait DiagSeverityExt {
    fn display_name(self) -> &'static str;
}

impl DiagSeverityExt for DiagSeverity {
    fn display_name(self) -> &'static str {
        match self {
            DiagSeverity::Error   => "ERROR",
            DiagSeverity::Warning => "WARNING",
            DiagSeverity::Note    => "NOTE",
            DiagSeverity::Help    => "HELP",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NormalizedDiag {
    pub severity: DiagSeverity,
    pub line: usize,
    pub message: String,
}

impl NormalizedDiag {
    pub fn from_glyim_diag(diag: &GlyimDiagnostic, source: &str) -> Self {
        let line = byte_offset_to_line(source, diag.span.primary.lo.to_usize());
        Self { severity: diag.severity, line, message: diag.message.clone() }
    }
}

#[derive(Clone, Debug)]
pub struct ComparisonResult {
    pub matched: Vec<MatchedPair>,
    pub missing: Vec<Annotation>,
    pub unexpected: Vec<NormalizedDiag>,
    pub wrong_severity: Vec<SeverityMismatch>,
    pub optional_unmatched: Vec<Annotation>,
}

impl ComparisonResult {
    pub fn passed(&self) -> bool {
        self.missing.is_empty()
            && self.unexpected.is_empty()
            && self.wrong_severity.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct MatchedPair { pub annotation: Annotation, pub diagnostic: NormalizedDiag }

#[derive(Clone, Debug)]
pub struct SeverityMismatch {
    pub annotation: Annotation,
    pub diagnostic: NormalizedDiag,
    pub expected: DiagSeverity,
    pub actual: DiagSeverity,
}

pub fn compare_diagnostics(
    annotations: &[Annotation],
    diagnostics: &[NormalizedDiag],
) -> ComparisonResult {
    let mut matched = Vec::new();
    let mut missing = Vec::new();
    let mut wrong_severity = Vec::new();
    let mut optional_unmatched = Vec::new();
    let mut diag_used = vec![false; diagnostics.len()];

    for annotation in annotations {
        let target_line = annotation.target_line();
        let mut found = false;

        for (i, diag) in diagnostics.iter().enumerate() {
            if diag_used[i] { continue; }

            let line_matches = if annotation.fuzzy {
                diag.line.abs_diff(target_line) <= 1
            } else {
                diag.line == target_line
            };

            if line_matches && annotation.pattern.matches(&diag.message) {
                diag_used[i] = true;
                found = true;
                if diag.severity == annotation.severity {
                    matched.push(MatchedPair { annotation: annotation.clone(), diagnostic: diag.clone() });
                } else {
                    wrong_severity.push(SeverityMismatch {
                        annotation: annotation.clone(), diagnostic: diag.clone(),
                        expected: annotation.severity, actual: diag.severity,
                    });
                }
                break;
            }
        }

        if !found {
            if annotation.optional {
                optional_unmatched.push(annotation.clone());
            } else {
                missing.push(annotation.clone());
            }
        }
    }

    let unexpected: Vec<NormalizedDiag> = diagnostics.iter().enumerate()
        .filter(|(i, _)| !diag_used[*i])
        .map(|(_, d)| d.clone())
        .collect();

    ComparisonResult { matched, missing, unexpected, wrong_severity, optional_unmatched }
}

fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())].chars().filter(|&c| c == '\n').count()
}
