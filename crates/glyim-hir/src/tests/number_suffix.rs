use glyim_core::primitives::*;
use glyim_span::FileId;
use glyim_frontend::parse_to_syntax;
use crate::lower::lower_literal;
use crate::Literal;

// Extract the literal token from a parsed literal expression
fn token_from_literal(src: &str) -> glyim_syntax::SyntaxToken {
    let parse = parse_to_syntax(src, FileId::from_raw(1));
    let lit_node = parse.root
        .children()
        .find(|n| n.kind() == glyim_syntax::SyntaxKind::LitExpr)
        .expect("LitExpr node not found");
    lit_node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind().is_literal())
        .expect("literal token not found")
        .clone()
}

#[test]
fn test_int_suffix_i32() {
    let tok = token_from_literal("42i32");
    let lit = lower_literal(&tok);
    assert_eq!(lit, Literal::Int(42, Some(IntTy::I32)));
}

#[test]
fn test_int_suffix_u64() {
    let tok = token_from_literal("100u64");
    let lit = lower_literal(&tok);
    assert_eq!(lit, Literal::Uint(100, Some(UintTy::U64)));
}

#[test]
fn test_int_hex_no_suffix() {
    let tok = token_from_literal("0x1A");
    let lit = lower_literal(&tok);
    assert_eq!(lit, Literal::Int(26, None));
}

#[test]
fn test_int_binary() {
    let tok = token_from_literal("0b1010");
    let lit = lower_literal(&tok);
    assert_eq!(lit, Literal::Int(10, None));
}

#[test]
fn test_float_suffix_f64() {
    let tok = token_from_literal("3.14f64");
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
    let tok = token_from_literal("2.71828");
    let lit = lower_literal(&tok);
    match lit {
        Literal::Float(bits, FloatTy::F64) => {
            let val = f64::from_bits(bits);
            assert!((val - 2.71828).abs() < 1e-6);
        }
        _ => panic!("expected Float f64"),
    }
}
