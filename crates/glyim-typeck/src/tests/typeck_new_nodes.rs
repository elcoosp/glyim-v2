use glyim_diag::GlyimDiagnostic;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;
use crate::{typeck_crate, TypeckResult};

// Helper to compile a source string and return typecheck result
fn typeck_source(src: &str) -> Vec<GlyimDiagnostic> {
    let file_id = FileId::from_raw(1);
    let parse = parse_to_syntax(src, file_id);
    // parse diagnostics are not relevant for typeck tests, but we assert none
    if !parse.diagnostics.is_empty() {
        return parse.diagnostics;
    }

    let krate = glyim_core::def_id::CrateId::from_raw(0);
    let (def_map, def_diags) = glyim_def_map::build_def_map(&parse.root, krate);
    if !def_diags.is_empty() {
        return def_diags;
    }

    let (hir, hir_diags) = glyim_hir::pipeline_api::lower_crate_for_pipeline(&parse.root, &mut glyim_core::Interner::new());
    if !hir_diags.is_empty() {
        return hir_diags;
    }

    let mut ctx = glyim_test::test_ty_ctx();
    let mut trait_ctx = glyim_solve::TraitContext::new();
    let mut solver = glyim_solve::SimpleTraitSolver::new(&trait_ctx);
    let (_frozen_ctx, typeck_result) = typeck_crate(ctx, &def_map, &hir, &mut solver);
    typeck_result.diagnostics
}

#[test]
fn match_guard_uses_binding() {
    let diags = typeck_source(
        r#"
        fn main() {
            let x = Some(5);
            match x {
                Some(y) if y > 0 => {},
                _ => {}
            }
        }
        "#,
    );
    assert!(diags.is_empty(), "expected no diagnostics, got {:?}", diags);
}

#[test]
fn or_pattern_same_types() {
    let diags = typeck_source(
        r#"
        fn main() {
            match 1 {
                1 | 2 => {},
                _ => {}
            }
        }
        "#,
    );
    assert!(diags.is_empty(), "expected no diagnostics, got {:?}", diags);
}

#[test]
fn range_pattern_integer() {
    let diags = typeck_source(
        r#"
        fn main() {
            match 5 {
                1..=10 => {},
                _ => {}
            }
        }
        "#,
    );
    assert!(diags.is_empty(), "expected no diagnostics, got {:?}", diags);
}

#[test]
fn slice_pattern_array() {
    let diags = typeck_source(
        r#"
        fn main() {
            let arr = [1, 2, 3];
            match arr {
                [a, b, ..rest] => {},
                _ => {}
            }
        }
        "#,
    );
    assert!(diags.is_empty(), "expected no diagnostics, got {:?}", diags);
}

#[test]
fn index_expression_array() {
    let diags = typeck_source(
        r#"
        fn main() {
            let arr = [1, 2, 3];
            let x = arr[0];
        }
        "#,
    );
    assert!(diags.is_empty(), "expected no diagnostics, got {:?}", diags);
}

#[test]
fn struct_literal_with_spread() {
    let diags = typeck_source(
        r#"
        struct S { x: i32, y: i32 }
        fn main() {
            let a = S { x: 1, y: 2 };
            let b = S { x: 3, ..a };
        }
        "#,
    );
    assert!(diags.is_empty(), "expected no diagnostics, got {:?}", diags);
}

#[test]
fn or_pattern_mismatched_types_fails() {
    let diags = typeck_source(
        r#"
        fn main() {
            match 1 {
                true | "hello" => {},
                _ => {}
            }
        }
        "#,
    );
    assert!(!diags.is_empty(), "expected diagnostics, got none");
    let diag_str = format!("{:?}", diags);
    assert!(diag_str.contains("mismatched types") || diag_str.contains("type error"),
            "expected mismatched types diagnostic, got {:?}", diags);
}
