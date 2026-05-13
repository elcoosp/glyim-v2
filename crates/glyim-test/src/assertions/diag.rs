use glyim_diag::{DiagSeverity, GlyimDiagnostic};

pub fn assert_no_errors(diagnostics: &[GlyimDiagnostic]) {
    let errors: Vec<_> = diagnostics.iter().filter(|d| d.is_error()).collect();
    assert!(errors.is_empty(), "expected no errors, found {}", errors.len());
}
pub fn assert_has_errors(diagnostics: &[GlyimDiagnostic]) {
    assert!(diagnostics.iter().any(|d| d.is_error()), "expected at least one error");
}
pub fn assert_error_count(diagnostics: &[GlyimDiagnostic], expected: usize) {
    let actual = diagnostics.iter().filter(|d| d.is_error()).count();
    assert_eq!(actual, expected);
}
pub fn assert_diag_contains(diagnostics: &[GlyimDiagnostic], substring: &str) {
    assert!(diagnostics.iter().any(|d| d.message.contains(substring)),
        "expected diagnostic containing {:?}", substring);
}
pub fn assert_diag_code(diagnostics: &[GlyimDiagnostic], code: &glyim_diag::ErrorCode) {
    assert!(diagnostics.iter().any(|d| d.code == *code),
        "expected diagnostic with code {:?}", code);
}
pub fn assert_has_severity(diagnostics: &[GlyimDiagnostic], severity: DiagSeverity) {
    assert!(diagnostics.iter().any(|d| d.severity == severity));
}
