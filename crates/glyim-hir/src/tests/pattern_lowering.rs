use crate::lower::lower_pat;
use crate::{Interner, Pat, PatId};
use glyim_core::arena::IndexVec;
use glyim_core::primitives::*;
use glyim_frontend::parse_to_syntax;
use glyim_span::Span;
use glyim_syntax::SyntaxKind;

/// Helper: parse source code, find the first pattern node, and lower it.
fn lower_first_pattern(source: &str) -> (Interner, IndexVec<PatId, Pat>, PatId) {
    let parse_result = parse_to_syntax(source, glyim_span::FileId::BOGUS);
    let root = parse_result.root;
    let mut interner = Interner::default();
    let pat_node = root
        .descendants()
        .find(|n| {
            matches!(
                n.kind(),
                SyntaxKind::PatIdent
                    | SyntaxKind::PatWild
                    | SyntaxKind::PatLit
                    | SyntaxKind::PatTuple
                    | SyntaxKind::PatStruct
                    | SyntaxKind::PatOr
                    | SyntaxKind::PatRange
                    | SyntaxKind::PatPath
            )
        })
        .expect("no pattern node found in source");

    let mut pats = IndexVec::new();
    let pat_id =
        lower_pat(&pat_node, &mut interner, &mut pats).expect("lower_pat returned None");
    (interner, pats, pat_id)
}

#[test]
fn u07_t01_lower_or_pattern() {
    let (interner, pats, pat_id) =
        lower_first_pattern("fn f() { match () { Some(x) | None => () } }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Or(arms) => {
            assert_eq!(arms.len(), 2, "Or should have 2 arms");
            let arm0 = &pats[arms[0]];
            match arm0 {
                Pat::Struct { path, fields, rest } => {
                    assert_eq!(path.segments.len(), 1);
                    assert_eq!(interner.resolve(path.segments[0].name), "Some");
                    assert_eq!(fields.len(), 1);
                    assert_eq!(interner.resolve(fields[0].0), "x");
                    let field_pat = &pats[fields[0].1];
                    assert!(
                        matches!(field_pat, Pat::Binding { name, .. } if interner.resolve(*name) == "x")
                    );
                    assert!(!rest, "no rest");
                }
                other => panic!("Expected PatStruct for first arm, got {:?}", other),
            }
            let arm1 = &pats[arms[1]];
            match arm1 {
                Pat::Path(path) => {
                    assert_eq!(path.segments.len(), 1);
                    assert_eq!(interner.resolve(path.segments[0].name), "None");
                }
                other => panic!("Expected PatPath for second arm, got {:?}", other),
            }
        }
        other => panic!("Expected Pat::Or, got {:?}", other),
    }
}

#[test]
fn u07_t02_lower_range_pattern() {
    let (interner, pats, pat_id) =
        lower_first_pattern("fn f() { match 0 { 0..=5 => () } }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Range {
            start,
            end,
            inclusive,
        } => {
            assert!(matches!(start, Some(Literal::Int(0, _))));
            assert!(matches!(end, Some(Literal::Int(5, _))));
            assert!(*inclusive, "range should be inclusive");
        }
        other => panic!("Expected Pat::Range, got {:?}", other),
    }
}

#[test]
fn u07_t03_lower_struct_pattern() {
    let (interner, pats, pat_id) = lower_first_pattern(
        "struct Point { x: i32, y: i32 } fn f(p: Point) { let Point { x, y: 0 } = p; }",
    );
    let pat = &pats[pat_id];
    match pat {
        Pat::Struct { path, fields, rest } => {
            assert_eq!(path.segments.len(), 1);
            assert_eq!(interner.resolve(path.segments[0].name), "Point");
            assert_eq!(fields.len(), 2);
            assert_eq!(interner.resolve(fields[0].0), "x");
            let x_pat = &pats[fields[0].1];
            assert!(
                matches!(x_pat, Pat::Binding { name, subpattern: None } if interner.resolve(*name) == "x")
            );
            assert_eq!(interner.resolve(fields[1].0), "y");
            let y_pat = &pats[fields[1].1];
            assert!(matches!(y_pat, Pat::Literal(Literal::Int(0, _))));
            assert!(!rest);
        }
        other => panic!("Expected Pat::Struct, got {:?}", other),
    }
}

#[test]
fn u07_t04_lower_nested_tuple_pattern() {
    let (interner, pats, pat_id) = lower_first_pattern(
        "enum Option<T> { Some(T), None } fn f(v: Option<(i32, i32)>) { match v { (a, Some(b)) => () } }",
    );
    let pat = &pats[pat_id];
    match pat {
        Pat::Tuple(elems) => {
            assert_eq!(elems.len(), 2);
            let a_pat = &pats[elems[0]];
            assert!(
                matches!(a_pat, Pat::Binding { name, .. } if interner.resolve(*name) == "a")
            );
            let some_pat = &pats[elems[1]];
            match some_pat {
                Pat::Struct { path, fields, rest } => {
                    assert_eq!(interner.resolve(path.segments[0].name), "Some");
                    assert_eq!(fields.len(), 1);
                    assert_eq!(interner.resolve(fields[0].0), "b");
                    let b_pat = &pats[fields[0].1];
                    assert!(
                        matches!(b_pat, Pat::Binding { name, .. } if interner.resolve(*name) == "b")
                    );
                    assert!(!rest);
                }
                other => panic!("Expected Pat::Struct for Some, got {:?}", other),
            }
        }
        other => panic!("Expected Pat::Tuple, got {:?}", other),
    }
}
