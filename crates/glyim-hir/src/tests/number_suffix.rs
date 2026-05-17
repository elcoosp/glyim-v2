use glyim_core::primitives::*;
use glyim_syntax::SyntaxToken;
use crate::lower::lower_literal;
use crate::Literal;

fn token_text(s: &str) -> SyntaxToken {
    use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
    use glyim_syntax::{GreenToken, SyntaxKind};
    let kind = if s.starts_with('"') {
        SyntaxKind::StringLit
    } else if s.starts_with('\'') {
        SyntaxKind::CharLit
    } else if s.contains('.') {
        SyntaxKind::FloatLit
    } else if s == "true" || s == "false" {
        SyntaxKind::BoolLit
    } else {
        SyntaxKind::IntLit
    };
    let green = GreenToken::new(kind, s.len() as u32, s.into());
    SyntaxToken::new(green, FileId::from_raw(1), ByteIdx::ZERO, SyntaxContext::ROOT)
}

#[test]
fn test_int_suffix_i32() {
    let tok = token_text("42i32");
    let lit = lower_literal(&tok);
    assert_eq!(lit, Literal::Int(42, Some(IntTy::I32)));
}

#[test]
fn test_int_suffix_u64() {
    let tok = token_text("100u64");
    let lit = lower_literal(&tok);
    assert_eq!(lit, Literal::Uint(100, Some(UintTy::U64)));
}

#[test]
fn test_int_hex_no_suffix() {
    let tok = token_text("0x1A");
    let lit = lower_literal(&tok);
    assert_eq!(lit, Literal::Int(26, None));
}

#[test]
fn test_int_binary() {
    let tok = token_text("0b1010");
    let lit = lower_literal(&tok);
    assert_eq!(lit, Literal::Int(10, None));
}

#[test]
fn test_float_suffix_f64() {
    let tok = token_text("3.14f64");
    let lit = lower_literal(&tok);
    match lit {
        Literal::Float(bits, FloatTy::F64) => {
            let val = f64::from_bits(bits);
            assert!((val - 3.14).abs() < 1e-6);
        }
        _ => panic!("expected Float f64"),
    }
}

#[test]
fn test_float_no_suffix() {
    let tok = token_text("2.71828");
    let lit = lower_literal(&tok);
    match lit {
        Literal::Float(bits, FloatTy::F64) => {
            let val = f64::from_bits(bits);
            assert!((val - 2.71828).abs() < 1e-6);
        }
        _ => panic!("expected Float f64"),
    }
}
