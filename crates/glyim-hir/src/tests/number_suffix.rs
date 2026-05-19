use crate::Literal;
use crate::lower::lower_literal;
use glyim_core::primitives::*;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;
use glyim_syntax::SyntaxKind;

fn token_from_literal(src: &str) -> glyim_syntax::SyntaxToken {
    let full_src = format!("fn main() {{ {} }}", src);
    let parse = parse_to_syntax(&full_src, FileId::from_raw(1));
    let fn_def = parse
        .root
        .children()
        .find(|n| n.kind() == SyntaxKind::FnDef)
        .expect("FnDef not found");
    let block = fn_def
        .children()
        .find(|n| n.kind() == SyntaxKind::Block)
        .expect("Block not found");
    // Look for ExprStmt containing the literal, or direct literal node
    let lit_expr = block
        .children()
        .find(|n| n.kind() == SyntaxKind::ExprStmt)
        .and_then(|stmt| stmt.children().find(|c| c.kind() == SyntaxKind::LitExpr))
        .or_else(|| block.children().find(|c| c.kind() == SyntaxKind::LitExpr))
        .expect("LitExpr not found");
    lit_expr
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
    assert_eq!(*lit, Literal::Int(42, Some(IntTy::I32)));
}

#[test]
fn test_int_suffix_u64() {
    let tok = token_from_literal("100u64");
    let lit = lower_literal(&tok);
    assert_eq!(*lit, Literal::Uint(100, Some(UintTy::U64)));
}

#[test]
fn test_int_hex_no_suffix() {
    let tok = token_from_literal("0x1A");
    let lit = lower_literal(&tok);
    assert_eq!(*lit, Literal::Int(26, None));
}

#[test]
fn test_int_binary() {
    let tok = token_from_literal("0b1010");
    let lit = lower_literal(&tok);
    assert_eq!(*lit, Literal::Int(10, None));
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
