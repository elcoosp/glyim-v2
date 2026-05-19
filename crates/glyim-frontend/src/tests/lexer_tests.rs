use crate::Token;
use crate::lexer::lex;
use glyim_span::FileId;
use glyim_syntax::SyntaxKind;
use glyim_test::assert_no_errors;

fn test_lex(source: &str) -> Vec<Token> {
    let file_id = FileId::from_raw(1);
    let result = lex(source, file_id);
    assert_no_errors(&result.diagnostics);
    result.tokens
}

#[test]
fn lex_simple_identifiers() {
    let tokens = test_lex("foo bar _baz");
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].kind, SyntaxKind::Ident);
    assert_eq!(tokens[0].text, "foo");
    assert_eq!(tokens[1].kind, SyntaxKind::Ident);
    assert_eq!(tokens[1].text, "bar");
    assert_eq!(tokens[2].kind, SyntaxKind::Ident);
    assert_eq!(tokens[2].text, "_baz");
}

#[test]
fn lex_keywords() {
    let tokens = test_lex("fn struct enum if else return");
    let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        vec![
            SyntaxKind::KwFn,
            SyntaxKind::KwStruct,
            SyntaxKind::KwEnum,
            SyntaxKind::KwIf,
            SyntaxKind::KwElse,
            SyntaxKind::KwReturn,
        ]
    );
}

#[test]
fn lex_integer_literals() {
    let tokens = test_lex("42 0x1A 0b101 0o777 0");
    assert_eq!(tokens.len(), 5);
    for t in &tokens {
        assert_eq!(t.kind, SyntaxKind::IntLit);
    }
    assert_eq!(tokens[0].text, "42");
    assert_eq!(tokens[1].text, "0x1A");
    assert_eq!(tokens[2].text, "0b101");
    assert_eq!(tokens[3].text, "0o777");
    assert_eq!(tokens[4].text, "0");
}

#[test]
fn lex_float_literals() {
    let tokens = test_lex("3.14 1.2e10 0.5E-3");
    assert_eq!(tokens.len(), 3);
    for t in &tokens {
        assert_eq!(t.kind, SyntaxKind::FloatLit);
    }
    assert_eq!(tokens[0].text, "3.14");
    assert_eq!(tokens[1].text, "1.2e10");
    assert_eq!(tokens[2].text, "0.5E-3");
}

#[test]
fn lex_string_and_char() {
    let tokens = test_lex("\"hello\" 'a' \"\\n\"");
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].kind, SyntaxKind::StringLit);
    assert_eq!(tokens[0].text, "\"hello\"");
    assert_eq!(tokens[1].kind, SyntaxKind::CharLit);
    assert_eq!(tokens[1].text, "'a'");
    assert_eq!(tokens[2].kind, SyntaxKind::StringLit);
    assert_eq!(tokens[2].text, "\"\\n\"");
}

#[test]
fn lex_punctuation() {
    let tokens = test_lex("(){}[] , ; . :: -> =>");
    let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        vec![
            SyntaxKind::LParen,
            SyntaxKind::RParen,
            SyntaxKind::LBrace,
            SyntaxKind::RBrace,
            SyntaxKind::LBracket,
            SyntaxKind::RBracket,
            SyntaxKind::Comma,
            SyntaxKind::Semicolon,
            SyntaxKind::Dot,
            SyntaxKind::ColonColon,
            SyntaxKind::Arrow,
            SyntaxKind::FatArrow,
        ]
    );
}

#[test]
fn lex_operators() {
    let tokens = test_lex("+ - * / % = == != < > <= >= && || ! ^ << >>");
    let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        vec![
            SyntaxKind::Plus,
            SyntaxKind::Minus,
            SyntaxKind::Star,
            SyntaxKind::Slash,
            SyntaxKind::Percent,
            SyntaxKind::Eq,
            SyntaxKind::EqEq,
            SyntaxKind::BangEq,
            SyntaxKind::Lt,
            SyntaxKind::Gt,
            SyntaxKind::LtEq,
            SyntaxKind::GtEq,
            SyntaxKind::AndAnd,
            SyntaxKind::OrOr,
            SyntaxKind::Bang,
            SyntaxKind::Caret,
            SyntaxKind::Shl,
            SyntaxKind::Shr,
        ]
    );
}

#[test]
fn lex_line_comment() {
    let tokens = test_lex("foo // comment\nbar");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].kind, SyntaxKind::Ident);
    assert_eq!(tokens[0].text, "foo");
    assert_eq!(tokens[1].kind, SyntaxKind::Ident);
    assert_eq!(tokens[1].text, "bar");
}

#[test]
fn lex_block_comment() {
    let tokens = test_lex("foo /* nested /* comment */ */ bar");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].kind, SyntaxKind::Ident);
    assert_eq!(tokens[0].text, "foo");
    assert_eq!(tokens[1].kind, SyntaxKind::Ident);
    assert_eq!(tokens[1].text, "bar");
}

#[test]
fn lex_lifetime() {
    let tokens = test_lex("'a: 'static");
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].kind, SyntaxKind::Lifetime);
    assert_eq!(tokens[0].text, "'a");
    assert_eq!(tokens[1].kind, SyntaxKind::Colon);
    assert_eq!(tokens[2].kind, SyntaxKind::Lifetime);
    assert_eq!(tokens[2].text, "'static");
}

#[test]
fn lex_number_suffix_valid() {
    let tokens = test_lex("42i32 100u8 3.14f64 0xABusize");
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0].kind, SyntaxKind::IntLit);
    assert_eq!(tokens[0].text, "42i32");
    assert_eq!(tokens[1].kind, SyntaxKind::IntLit);
    assert_eq!(tokens[1].text, "100u8");
    assert_eq!(tokens[2].kind, SyntaxKind::FloatLit);
    assert_eq!(tokens[2].text, "3.14f64");
    assert_eq!(tokens[3].kind, SyntaxKind::IntLit);
    assert_eq!(tokens[3].text, "0xABusize");
}
