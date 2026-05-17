use glyim_core::interner::Interner;
use glyim_span::FileId;
use glyim_frontend::parse_to_syntax;
use glyim_syntax::SyntaxNode;
use crate::lower::lower_expr;
use crate::{Expr, Pat, TypeRef};

fn parse_expr(src: &str) -> SyntaxNode {
    let parse = parse_to_syntax(src, FileId::from_raw(1)).unwrap();
    parse.root
        .children()
        .find(|n| n.kind().is_expr())
        .expect("expr node not found")
        .clone()
}

#[test]
fn test_lower_closure_expr() {
    let node = parse_expr("|x: i32| x + 1");
    let mut interner = Interner::default();
    let mut exprs = glyim_core::arena::IndexVec::new();
    let mut pats = glyim_core::arena::IndexVec::new();
    let mut expr_spans = glyim_core::arena::IndexVec::new();
    let eid = lower_expr(&node, &mut interner, &mut exprs, &mut pats, &mut expr_spans).unwrap();
    match &exprs[eid] {
        Expr::Closure { params, body: _ } => {
            assert_eq!(params.len(), 1);
            match &pats[params[0]] {
                Pat::Binding { name, .. } => {
                    let resolved = interner.resolve(*name).unwrap();
                    assert_eq!(resolved, "x");
                }
                _ => panic!("expected Binding pattern"),
            }
        }
        _ => panic!("expected Closure expr"),
    }
}

#[test]
fn test_lower_struct_expr() {
    let node = parse_expr("Point { x: 10, y: 20 }");
    let mut interner = Interner::default();
    let mut exprs = glyim_core::arena::IndexVec::new();
    let mut pats = glyim_core::arena::IndexVec::new();
    let mut expr_spans = glyim_core::arena::IndexVec::new();
    let eid = lower_expr(&node, &mut interner, &mut exprs, &mut pats, &mut expr_spans).unwrap();
    match &exprs[eid] {
        Expr::Struct { path, fields, spread } => {
            assert_eq!(path.as_name(), Some(interner.intern("Point")));
            assert_eq!(fields.len(), 2);
            let resolved0 = interner.resolve(fields[0].0).unwrap();
            let resolved1 = interner.resolve(fields[1].0).unwrap();
            assert_eq!(resolved0, "x");
            assert_eq!(resolved1, "y");
            assert!(spread.is_none());
        }
        _ => panic!("expected Struct expr"),
    }
}

#[test]
fn test_lower_range_expr() {
    let node = parse_expr("1..10");
    let mut interner = Interner::default();
    let mut exprs = glyim_core::arena::IndexVec::new();
    let mut pats = glyim_core::arena::IndexVec::new();
    let mut expr_spans = glyim_core::arena::IndexVec::new();
    let eid = lower_expr(&node, &mut interner, &mut exprs, &mut pats, &mut expr_spans).unwrap();
    match &exprs[eid] {
        Expr::Range { start, end, inclusive } => {
            assert!(start.is_some());
            assert!(end.is_some());
            assert!(!*inclusive);
        }
        _ => panic!("expected Range expr"),
    }
}

#[test]
fn test_lower_index_expr() {
    let node = parse_expr("arr[0]");
    let mut interner = Interner::default();
    let mut exprs = glyim_core::arena::IndexVec::new();
    let mut pats = glyim_core::arena::IndexVec::new();
    let mut expr_spans = glyim_core::arena::IndexVec::new();
    let eid = lower_expr(&node, &mut interner, &mut exprs, &mut pats, &mut expr_spans).unwrap();
    match &exprs[eid] {
        Expr::Index { base, index } => {
            match &exprs[*base] {
                Expr::Path(p) => assert_eq!(p.as_name(), Some(interner.intern("arr"))),
                _ => panic!("expected Path base"),
            }
            match &exprs[*index] {
                Expr::Literal(lit) => match lit {
                    crate::Literal::Int(0, None) => {}
                    _ => panic!("expected Int literal 0"),
                },
                _ => panic!("expected Literal index"),
            }
        }
        _ => panic!("expected Index expr"),
    }
}

#[test]
fn test_lower_cast_expr() {
    let node = parse_expr("x as i32");
    let mut interner = Interner::default();
    let mut exprs = glyim_core::arena::IndexVec::new();
    let mut pats = glyim_core::arena::IndexVec::new();
    let mut expr_spans = glyim_core::arena::IndexVec::new();
    let eid = lower_expr(&node, &mut interner, &mut exprs, &mut pats, &mut expr_spans).unwrap();
    match &exprs[eid] {
        Expr::Cast { expr, ty } => {
            match &exprs[*expr] {
                Expr::Path(p) => assert_eq!(p.as_name(), Some(interner.intern("x"))),
                _ => panic!("expected Path expr"),
            }
            match ty {
                TypeRef::Path(p) => assert_eq!(p.as_name(), Some(interner.intern("i32"))),
                _ => panic!("expected Path type"),
            }
        }
        _ => panic!("expected Cast expr"),
    }
}
