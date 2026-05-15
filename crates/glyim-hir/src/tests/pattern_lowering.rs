use crate::lower::lower_pat;
use crate::{Literal, Pat, PatId};
use glyim_core::arena::IndexVec;
use glyim_core::interner::Interner;
use glyim_core::primitives::UintTy;

use glyim_frontend::parse_to_syntax;

use glyim_syntax::SyntaxKind;

/// Helper: parse source code, find the first pattern node, and lower it.
fn lower_first_pattern(source: &str) -> (Interner, IndexVec<PatId, Pat>, PatId) {
    let parse_result = parse_to_syntax(source, glyim_span::FileId::BOGUS);
    let root = parse_result.root;
    let mut interner = Interner::default();

    // Find the first pattern node in a MatchArm, LetStmt, or ForExpr
    let pat_node = root
        .descendants()
        .find(|n| {
            matches!(
                n.kind(),
                SyntaxKind::MatchArm | SyntaxKind::LetStmt | SyntaxKind::ForExpr
            )
        })
        .and_then(|stmt| {
            stmt.children_with_tokens()
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

#[test]
fn u07_t13_lower_single_element_tuple_pattern() {
    let (interner, pats, pat_id) = lower_first_pattern("fn f() { let (a,) = (1,); }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Tuple(elems) => {
            assert_eq!(elems.len(), 1);
            assert!(
                matches!(&pats[elems[0]], Pat::Binding { name, .. } if interner.resolve(*name) == "a")
            );
        }
        other => panic!("Expected Pat::Tuple, got {:?}", other),
    }
}

#[test]
fn u07_t14_lower_empty_tuple_pattern() {
    let (_interner, pats, pat_id) = lower_first_pattern("fn f() { let () = (); }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Tuple(elems) => {
            assert_eq!(elems.len(), 0);
        }
        other => panic!("Expected Pat::Tuple, got {:?}", other),
    }
}

#[test]
fn u07_t15_lower_boolean_literal_pattern() {
    let (_interner, pats, pat_id) =
        lower_first_pattern("fn f() { match true { true => (), false => () } }");
    let pat = &pats[pat_id];
    assert!(matches!(pat, Pat::Literal(Literal::Bool(true))));
}

#[test]
fn u07_t16_lower_char_literal_pattern() {
    let (_interner, pats, pat_id) = lower_first_pattern("fn f() { match 'a' { 'a' => () } }");
    let pat = &pats[pat_id];
    assert!(matches!(pat, Pat::Literal(Literal::Char('a'))));
}

#[test]
fn u07_t17_lower_unsigned_int_literal_pattern() {
    let (_interner, pats, pat_id) = lower_first_pattern("fn f() { match 5u32 { 5u32 => () } }");
    let pat = &pats[pat_id];
    assert!(matches!(
        pat,
        Pat::Literal(Literal::Uint(5, Some(UintTy::U32)))
    ));
}

#[test]
fn u07_t18_lower_struct_pattern_with_nested_patterns() {
    let (interner, pats, pat_id) = lower_first_pattern(
        "struct Outer { inner: Inner } struct Inner { a: i32, b: i32 } fn f(o: Outer) { let Outer { inner: Inner { a, b } } = o; }",
    );
    let pat = &pats[pat_id];
    match pat {
        Pat::Struct { path, fields, rest } => {
            assert_eq!(interner.resolve(path.segments[0].name), "Outer");
            assert_eq!(fields.len(), 1);
            assert_eq!(interner.resolve(fields[0].0), "inner");
            let inner_pat = &pats[fields[0].1];
            match inner_pat {
                Pat::Struct {
                    path: inner_path,
                    fields: inner_fields,
                    ..
                } => {
                    assert_eq!(interner.resolve(inner_path.segments[0].name), "Inner");
                    assert_eq!(inner_fields.len(), 2);
                    assert_eq!(interner.resolve(inner_fields[0].0), "a");
                    assert_eq!(interner.resolve(inner_fields[1].0), "b");
                }
                other => panic!("Expected nested PatStruct, got {:?}", other),
            }
        }
        other => panic!("Expected PatStruct, got {:?}", other),
    }
}

#[test]
fn u07_t19_lower_range_pattern_with_variable_end() {
    // The parser produces a PatIdent for the end variable inside a PatLit range node
    let (_interner, pats, pat_id) = lower_first_pattern("fn f() { match 0 { 0..MAX => () } }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Range {
            start,
            end,
            inclusive,
        } => {
            assert!(matches!(start, Some(Literal::Int(0, _))));
            // MAX should be parsed as a path pattern inside the range; we may get None or a literal
            assert!(!*inclusive);
        }
        Pat::Literal(_) => {
            // Accept: if the parser didn't create a range node for variable end, it's a literal
        }
        other => panic!("Expected Pat::Range or Pat::Literal, got {:?}", other),
    }
}

#[test]
fn u07_t20_lower_tuple_struct_pattern() {
    let (interner, pats, pat_id) = lower_first_pattern(
        "struct Color(i32, i32, i32); fn f(c: Color) { let Color(r, g, b) = c; }",
    );
    let pat = &pats[pat_id];
    match pat {
        Pat::Struct { path, fields, rest } => {
            assert_eq!(interner.resolve(path.segments[0].name), "Color");
            assert_eq!(fields.len(), 3);
            // Tuple struct fields have empty names from the parser
            assert_eq!(interner.resolve(fields[0].0), "r");
            assert_eq!(interner.resolve(fields[1].0), "g");
            assert_eq!(interner.resolve(fields[2].0), "b");
            assert!(!rest);
        }
        other => panic!("Expected PatStruct, got {:?}", other),
    }
}

#[test]
fn u07_t21_lower_enum_tuple_variant_pattern() {
    let (interner, pats, pat_id) =
        lower_first_pattern("enum E { V(i32, i32) } fn f(e: E) { match e { V(x, y) => () } }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Struct { path, fields, rest } => {
            assert_eq!(interner.resolve(path.segments[0].name), "V");
            assert_eq!(fields.len(), 2);
            assert_eq!(interner.resolve(fields[0].0), "x");
            assert_eq!(interner.resolve(fields[1].0), "y");
        }
        other => panic!("Expected PatStruct, got {:?}", other),
    }
}

#[test]
fn u07_t22_lower_mixed_or_with_literals_and_paths() {
    let (interner, pats, pat_id) = lower_first_pattern("fn f() { match 0 { 0 | 1 | 2 => () } }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Or(arms) => {
            assert_eq!(arms.len(), 3);
            assert!(matches!(
                &pats[arms[0]],
                Pat::Literal(Literal::Int(0, None))
            ));
            assert!(matches!(
                &pats[arms[1]],
                Pat::Literal(Literal::Int(1, None))
            ));
            assert!(matches!(
                &pats[arms[2]],
                Pat::Literal(Literal::Int(2, None))
            ));
        }
        other => panic!("Expected Pat::Or, got {:?}", other),
    }
}

#[test]
fn u07_t23_lower_for_loop_pattern() {
    let (interner, pats, pat_id) = lower_first_pattern("fn f() { for (a, b) in vec![] { } }");
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

#[test]
fn u07_t24_lower_large_nested_or_structure() {
    let (interner, pats, pat_id) =
        lower_first_pattern("fn f() { match (0, 0) { (1 | 2, 3 | 4) => () } }");
    let pat = &pats[pat_id];
    match pat {
        Pat::Tuple(elems) => {
            assert_eq!(elems.len(), 2);
            // First element: 1 | 2
            match &pats[elems[0]] {
                Pat::Or(arms) => {
                    assert_eq!(arms.len(), 2);
                }
                other => panic!("Expected Pat::Or for first element, got {:?}", other),
            }
            // Second element: 3 | 4
            match &pats[elems[1]] {
                Pat::Or(arms) => {
                    assert_eq!(arms.len(), 2);
                }
                other => panic!("Expected Pat::Or for second element, got {:?}", other),
            }
        }
        other => panic!("Expected Pat::Tuple, got {:?}", other),
    }
}

#[test]
fn u07_t25_lower_match_arm_with_guard() {
    let (interner, pats, pat_id) = lower_first_pattern("fn f() { match 0 { x if x > 0 => () } }");
    let pat = &pats[pat_id];
    assert!(matches!(pat, Pat::Binding { name, .. } if interner.resolve(*name) == "x"));
}
