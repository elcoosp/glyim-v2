use crate::*;

#[test]
fn test_annotation_parser_exact_vs_fuzzy() {
    let exact_src = "fn main() {} //~ ERROR msg";
    let anns = annotations::Annotation::parse_all(exact_src).unwrap();
    assert_eq!(anns.len(), 1);
    assert!(!anns[0].fuzzy, "//~ must be exact");

    let fuzzy_src = "fn main() {} //~~ ERROR msg";
    let anns = annotations::Annotation::parse_all(fuzzy_src).unwrap();
    assert_eq!(anns.len(), 1);
    assert!(anns[0].fuzzy, "//~~ must be fuzzy");
}

#[test]
fn test_annotation_caret_offset() {
    let src = "fn main() {} //~^^ ERROR msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns.len(), 1);
    assert_eq!(anns[0].line_offset, 2);
    assert_eq!(anns[0].target_line(), 0);
}

#[test]
fn test_annotation_optional() {
    let src = "fn main() {} //~? ERROR msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns.len(), 1);
    assert!(anns[0].optional);
}

#[test]
fn test_annotation_continuation() {
    let src = "line1\nline2\nline3 //~ ERROR msg\n     //~| NOTE sub";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns.len(), 2);
    assert_eq!(anns[0].target_line(), anns[1].target_line());
}

#[test]
fn test_annotation_severity_default_error() {
    let src = "fn main() {} //~ msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns[0].severity, glyim_diag::DiagSeverity::Error);
}

#[test]
fn test_annotation_severity_warning() {
    let src = "fn main() {} //~ WARNING msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns[0].severity, glyim_diag::DiagSeverity::Warning);
}

#[test]
fn test_annotation_severity_note() {
    let src = "fn main() {} //~ NOTE msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns[0].severity, glyim_diag::DiagSeverity::Note);
}

#[test]
fn test_annotation_severity_help() {
    let src = "fn main() {} //~ HELP msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns[0].severity, glyim_diag::DiagSeverity::Help);
}

#[test]
fn test_annotation_invalid_severity_treated_as_message() {
    let anns = annotations::Annotation::parse_all("fn main() {} //~ ERRR msg").unwrap();
    assert_eq!(anns.len(), 1);
    assert_eq!(anns[0].severity, glyim_diag::DiagSeverity::Error);
    assert!(matches!(
        anns[0].pattern,
        annotations::pattern::MatchPattern::Substring(_)
    ));
    assert_eq!(anns[0].pattern.description(), "contains \"ERRR msg\"");
}

#[test]
fn test_match_pattern_any() {
    let p = annotations::pattern::MatchPattern::Any;
    assert!(p.matches("anything"));
    assert_eq!(p.description(), "<any>");
}

#[test]
fn test_match_pattern_substring() {
    let p = annotations::pattern::MatchPattern::substring("hello");
    assert!(p.matches("say hello world"));
    assert!(!p.matches("say world"));
}

#[test]
fn test_match_pattern_exact() {
    let p = annotations::pattern::MatchPattern::exact("hello");
    assert!(p.matches("hello"));
    assert!(!p.matches("hello world"));
}

#[test]
fn test_match_pattern_regex() {
    let p = annotations::pattern::MatchPattern::regex("error|warning").unwrap();
    assert!(p.matches("got an error here"));
    assert!(p.matches("got a warning here"));
    assert!(!p.matches("got a note here"));
}
