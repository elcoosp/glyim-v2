use crate::*;

#[test]
fn test_comparison_invariant_with_optional() {
    use annotations::Annotation;
    use annotations::pattern::MatchPattern;
    use comparison;
    use glyim_diag::DiagSeverity;

    let ann = Annotation {
        line: 0,
        line_offset: 0,
        severity: DiagSeverity::Error,
        pattern: MatchPattern::Any,
        optional: true,
        fuzzy: false,
    };
    let result = comparison::compare_diagnostics(&[ann], &[]);
    assert!(result.passed());
    assert_eq!(result.optional_unmatched.len(), 1);
}

#[test]
fn test_comparison_exact_match() {
    use annotations::Annotation;
    use annotations::pattern::MatchPattern;
    use comparison;
    use glyim_diag::DiagSeverity;

    let ann = Annotation {
        line: 0,
        line_offset: 0,
        severity: DiagSeverity::Error,
        pattern: MatchPattern::Any,
        optional: false,
        fuzzy: false,
    };
    let diag = comparison::NormalizedDiag {
        severity: DiagSeverity::Error,
        line: 0,
        message: "test".into(),
    };
    let result = comparison::compare_diagnostics(&[ann], &[diag]);
    assert!(result.passed());
    assert_eq!(result.matched.len(), 1);
}

#[test]
fn test_comparison_wrong_severity() {
    use annotations::Annotation;
    use annotations::pattern::MatchPattern;
    use comparison;
    use glyim_diag::DiagSeverity;

    let ann = Annotation {
        line: 0,
        line_offset: 0,
        severity: DiagSeverity::Error,
        pattern: MatchPattern::Any,
        optional: false,
        fuzzy: false,
    };
    let diag = comparison::NormalizedDiag {
        severity: DiagSeverity::Warning,
        line: 0,
        message: "test".into(),
    };
    let result = comparison::compare_diagnostics(&[ann], &[diag]);
    assert!(!result.passed());
    assert_eq!(result.wrong_severity.len(), 1);
}

#[test]
fn test_comparison_fuzzy_match() {
    use annotations::Annotation;
    use annotations::pattern::MatchPattern;
    use comparison;
    use glyim_diag::DiagSeverity;

    let ann = Annotation {
        line: 5,
        line_offset: 0,
        severity: DiagSeverity::Error,
        pattern: MatchPattern::Any,
        optional: false,
        fuzzy: true,
    };
    let diag = comparison::NormalizedDiag {
        severity: DiagSeverity::Error,
        line: 6,
        message: "test".into(),
    };
    let result = comparison::compare_diagnostics(&[ann], &[diag]);
    assert!(result.passed());
}

#[test]
fn test_comparison_unexpected_diagnostic() {
    use comparison;
    use glyim_diag::DiagSeverity;

    let diag = comparison::NormalizedDiag {
        severity: DiagSeverity::Error,
        line: 0,
        message: "unexpected".into(),
    };
    let result = comparison::compare_diagnostics(&[], &[diag]);
    assert!(!result.passed());
    assert_eq!(result.unexpected.len(), 1);
}

#[test]
fn test_comparison_substring_pattern() {
    use annotations::Annotation;
    use annotations::pattern::MatchPattern;
    use comparison;
    use glyim_diag::DiagSeverity;

    let ann = Annotation {
        line: 0,
        line_offset: 0,
        severity: DiagSeverity::Error,
        pattern: MatchPattern::substring("mismatch"),
        optional: false,
        fuzzy: false,
    };
    let diag = comparison::NormalizedDiag {
        severity: DiagSeverity::Error,
        line: 0,
        message: "type mismatch: expected i32".into(),
    };
    let result = comparison::compare_diagnostics(&[ann], &[diag]);
    assert!(result.passed());
}

#[test]
fn test_normalize_output() {
    let rules = comparison::normalize::NormalizeRules {
        normalize_line_endings: true,
        normalize_slashes: true,
        substitute_dir: false,
    };
    let result = comparison::normalize::normalize_output(
        "hello\r\nworld\\path",
        std::path::Path::new("test.g"),
        &rules,
    );
    assert_eq!(result, "hello\nworld/path");
}

#[test]
fn test_normalize_substitute_dir() {
    let rules = comparison::normalize::NormalizeRules {
        normalize_line_endings: false,
        normalize_slashes: false,
        substitute_dir: true,
    };
    let dir = std::path::Path::new("/some/dir/test.g");
    let result = comparison::normalize::normalize_output("error at /some/dir/file.g", dir, &rules);
    assert!(result.contains("$DIR"));
}

#[test]
fn test_diag_severity_ext() {
    use comparison::DiagSeverityExt;
    assert_eq!(glyim_diag::DiagSeverity::Error.display_name(), "ERROR");
    assert_eq!(glyim_diag::DiagSeverity::Warning.display_name(), "WARNING");
    assert_eq!(glyim_diag::DiagSeverity::Note.display_name(), "NOTE");
    assert_eq!(glyim_diag::DiagSeverity::Help.display_name(), "HELP");
}
