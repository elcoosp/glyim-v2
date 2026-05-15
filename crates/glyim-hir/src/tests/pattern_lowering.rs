use crate::lower::lower_pat;
use crate::{Literal, Pat, PatId};
use glyim_core::arena::IndexVec;
use glyim_core::interner::Interner;

use glyim_frontend::parse_to_syntax;

use glyim_syntax::SyntaxKind;

/// Helper: parse source code, find the first pattern node, and lower it.
fn lower_first_pattern(source: &str) -> (Interner, IndexVec<PatId, Pat>, PatId) {
    let parse_result = parse_to_syntax(source, glyim_span::FileId::BOGUS);
    let root = parse_result.root;
    let mut interner = Interner::default();
    // Find the first match arm pattern: look for MatchArm node, then get its first child before the fat arrow
    let pat_node = root
        .descendants()
        .find(|n| n.kind() == SyntaxKind::MatchArm)
        .and_then(|arm| {
            arm.children_with_tokens()
                .filter_map(|el| el.into_node())
                .find(|child| {
                    matches!(
                        child.kind(),
                        SyntaxKind::PatIdent
                            | SyntaxKind::PatWild
                            | SyntaxKind::PatLit
                            | SyntaxKind::PatTuple
                            | SyntaxKind::PatStruct
                            | SyntaxKind::PatOr
                            | SyntaxKind::PathExpr // e.g., None
                    )
                })
        })
        .or_else(|| {
            // Fallback: look for a LetStmt and get its pattern
            root.descendants()
                .find(|n| n.kind() == SyntaxKind::LetStmt)
                .and_then(|let_stmt| {
                    let_stmt
                        .children_with_tokens()
                        .filter_map(|el| el.into_node())
                        .find(|child| {
                            matches!(
                                child.kind(),
                                SyntaxKind::PatIdent
                                    | SyntaxKind::PatWild
                                    | SyntaxKind::PatLit
                                    | SyntaxKind::PatTuple
                                    | SyntaxKind::PatStruct
                                    | SyntaxKind::PatOr
                            )
                        })
                })
        })
        .expect("no pattern node found in source");

    let mut pats = IndexVec::new();
    let pat_id = lower_pat(&pat_node, &mut interner, &mut pats).expect("lower_pat returned None");
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
    let (interner, pats, pat_id) = lower_first_pattern("fn f() { match 0 { 0..=5 => () } }");
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
                matches!(x_pat, Pat::Binding { name, subpattern: None, .. } if interner.resolve(*name) == "x")
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
            assert!(matches!(a_pat, Pat::Binding { name, .. } if interner.resolve(*name) == "a"));
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

#[test]
fn u07_t05_lower_wildcard_pattern() {
    let (interner, pats, pat_id) = lower_first_pattern("fn f() { match () { _ => () } }");
    let pat = &pats[pat_id];
    assert!(matches!(pat, Pat::Wild));
}

#[test]
fn u07_t06_lower_literal_pattern() {
    let (interner, pats, pat_id) = lower_first_pattern("fn f() { match 0 { 42 => () } }");
    let pat = &pats[pat_id];
    assert!(matches!(pat, Pat::Literal(Literal::Int(42, None))));
}

#[test]
fn u07_t07_lower_multiple_or_arms() {
    let (interner, pats, pat_id) = lower_first_pattern("fn f() { match () { A | B | C => () } }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Or(arms) => {
            assert_eq!(arms.len(), 3);
            assert!(
                matches!(&pats[arms[0]], Pat::Path(p) if interner.resolve(p.segments[0].name) == "A")
            );
            assert!(
                matches!(&pats[arms[1]], Pat::Path(p) if interner.resolve(p.segments[0].name) == "B")
            );
            assert!(
                matches!(&pats[arms[2]], Pat::Path(p) if interner.resolve(p.segments[0].name) == "C")
            );
        }
        other => panic!("Expected Pat::Or, got {:?}", other),
    }
}

#[test]
fn u07_t08_lower_struct_pattern_with_rest() {
    let (interner, pats, pat_id) =
        lower_first_pattern("struct S { a: i32, b: i32 } fn f(s: S) { let S { a, .. } = s; }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Struct { path, fields, rest } => {
            assert_eq!(interner.resolve(path.segments[0].name), "S");
            assert_eq!(fields.len(), 1);
            assert_eq!(interner.resolve(fields[0].0), "a");
            let a_pat = &pats[fields[0].1];
            assert!(matches!(a_pat, Pat::Binding { name, .. } if interner.resolve(*name) == "a"));
            assert!(*rest, "rest should be true");
        }
        other => panic!("Expected Pat::Struct, got {:?}", other),
    }
}

#[test]
fn u07_t09_lower_path_pattern() {
    let (interner, pats, pat_id) = lower_first_pattern(
        "enum Option<T> { Some(T), None } fn f(v: Option<i32>) { match v { None => () } }",
    );
    let pat = &pats[pat_id];
    match pat {
        Pat::Path(path) => {
            assert_eq!(interner.resolve(path.segments[0].name), "None");
        }
        other => panic!("Expected Pat::Path, got {:?}", other),
    }
}

#[test]
fn u07_t10_lower_nested_or_in_tuple() {
    let (interner, pats, pat_id) =
        lower_first_pattern("fn f() { match (0, 0) { (1 | 2, 3) => () } }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Tuple(elems) => {
            assert_eq!(elems.len(), 2);
            let first = &pats[elems[0]];
            match first {
                Pat::Or(arms) => {
                    assert_eq!(arms.len(), 2);
                    assert!(matches!(
                        &pats[arms[0]],
                        Pat::Literal(Literal::Int(1, None))
                    ));
                    assert!(matches!(
                        &pats[arms[1]],
                        Pat::Literal(Literal::Int(2, None))
                    ));
                }
                other => panic!("Expected Pat::Or, got {:?}", other),
            }
            let second = &pats[elems[1]];
            assert!(matches!(second, Pat::Literal(Literal::Int(3, None))));
        }
        other => panic!("Expected Pat::Tuple, got {:?}", other),
    }
}

#[test]
fn u07_t11_lower_exclusive_range() {
    let (interner, pats, pat_id) = lower_first_pattern("fn f() { match 0 { 0..5 => () } }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Range {
            start,
            end,
            inclusive,
        } => {
            assert!(matches!(start, Some(Literal::Int(0, _))));
            assert!(matches!(end, Some(Literal::Int(5, _))));
            assert!(!*inclusive, "range should be exclusive");
        }
        other => panic!("Expected Pat::Range, got {:?}", other),
    }
}

#[test]
fn u07_t12_lower_let_tuple_pattern() {
    let (interner, pats, pat_id) = lower_first_pattern("fn f() { let (a, b) = (1, 2); }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Tuple(elems) => {
            assert_eq!(elems.len(), 2);
            assert!(
                matches!(&pats[elems[0]], Pat::Binding { name, .. } if interner.resolve(*name) == "a")
            );
            assert!(
                matches!(&pats[elems[1]], Pat::Binding { name, .. } if interner.resolve(*name) == "b")
            );
        }
        other => panic!("Expected Pat::Tuple, got {:?}", other),
    }
}
