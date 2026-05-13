use crate::lexer::lex;
use glyim_span::FileId;
use glyim_syntax::SyntaxKind;

#[test]
fn token_spans_are_contiguous() {
    let source = "fn main() {}";
    let result = lex(source, FileId::from_raw(0));
    let mut expected_start = 0u32;
    for token in &result.tokens {
        let (lo, hi) = token.span.range();
        assert_eq!(
            lo.to_raw(),
            expected_start,
            "token '{}' span should start at {}, got {}",
            token.text,
            expected_start,
            lo.to_raw()
        );
        let text_len = token.text.len() as u32;
        assert_eq!(
            hi.to_raw(),
            expected_start + text_len,
            "token '{}' span end mismatch",
            token.text
        );
        expected_start = hi.to_raw();
    }
}

#[test]
fn spans_cover_entire_source() {
    let source = "fn main() { let x = 42; }";
    let result = lex(source, FileId::from_raw(0));
    if let Some(last) = result.tokens.last() {
        let (_, hi) = last.span.range();
        assert_eq!(
            hi.to_raw() as usize,
            source.len(),
            "last token span end should equal source length"
        );
    }
}

#[test]
fn span_file_id_preserved() {
    let fid = FileId::from_raw(42);
    let result = lex("fn", fid);
    assert_eq!(result.tokens[0].span.file, fid);
}

#[test]
fn empty_input_no_tokens_no_diagnostics() {
    let result = lex("", FileId::from_raw(0));
    assert!(result.tokens.is_empty(), "empty input should produce no tokens");
    assert!(
        result.diagnostics.is_empty(),
        "empty input should produce no diagnostics"
    );
}

#[test]
fn whitespace_only_no_tokens_no_diagnostics() {
    let result = lex("   \n\t\r  ", FileId::from_raw(0));
    assert!(
        result.tokens.is_empty(),
        "whitespace-only input should produce no tokens"
    );
    assert!(
        result.diagnostics.is_empty(),
        "whitespace-only input should produce no diagnostics"
    );
}

#[test]
fn comment_only_no_tokens_no_diagnostics() {
    let result = lex("// just a comment\n/* block */", FileId::from_raw(0));
    assert!(
        result.tokens.is_empty(),
        "comment-only input should produce no tokens"
    );
    assert!(
        result.diagnostics.is_empty(),
        "comment-only input should produce no diagnostics"
    );
}

#[test]
fn span_for_multibyte_source() {
    let result = lex("1 2 3", FileId::from_raw(0));
    assert_eq!(result.tokens.len(), 3);
    assert_eq!(result.tokens[0].text, "1");
    assert_eq!(result.tokens[0].span.range().0.to_raw(), 0);
    assert_eq!(result.tokens[0].span.range().1.to_raw(), 1);

    assert_eq!(result.tokens[1].text, "2");
    assert_eq!(result.tokens[1].span.range().0.to_raw(), 2);
    assert_eq!(result.tokens[1].span.range().1.to_raw(), 3);

    assert_eq!(result.tokens[2].text, "3");
    assert_eq!(result.tokens[2].span.range().0.to_raw(), 4);
    assert_eq!(result.tokens[2].span.range().1.to_raw(), 5);
}
