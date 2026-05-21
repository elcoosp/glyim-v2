use glyim_def_map::{build_def_map, CrateDefMap};
use glyim_diag::GlyimDiagnostic;
use glyim_frontend::parse_to_syntax;
use glyim_hir::CrateHir;
use glyim_solve::{SimpleTraitSolver, TraitContext};
use glyim_span::FileId;
use glyim_type::TyCtxMut;
use glyim_typeck::{typeck_crate, TypeckResult};
use glyim_typeck::thir;

// Helper to compile a source string and return typecheck result
fn typeck_source(src: &str) -> (TyCtxMut, CrateDefMap, CrateHir, TypeckResult, Vec<GlyimDiagnostic>) {
    let file_id = FileId::from_raw(1);
    let parse = parse_to_syntax(src, file_id);
    assert!(parse.diagnostics.is_empty(), "parse errors: {:?}", parse.diagnostics);

    let krate = glyim_core::def_id::CrateId::from_raw(0);
    let (def_map, def_diags) = build_def_map(&parse.root, krate);
    assert!(def_diags.is_empty(), "def map errors: {:?}", def_diags);

    let (hir, hir_diags) = glyim_hir::pipeline_api::lower_crate_for_pipeline(&parse.root, &mut glyim_core::Interner::new());
    assert!(hir_diags.is_empty(), "hir lowering errors: {:?}", hir_diags);

    let mut ctx = glyim_test::test_ty_ctx();
    let mut trait_ctx = TraitContext::new();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let (frozen_ctx, typeck_result) = typeck_crate(ctx, &def_map, &hir, &mut solver);
    (frozen_ctx, def_map, hir, typeck_result, typeck_result.diagnostics)
}

#[test]
fn match_guard_uses_binding() {
    let (_ctx, _def_map, _hir, _typeck, diags) = typeck_source(
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
    let (_ctx, _def_map, _hir, _typeck, diags) = typeck_source(
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
    let (_ctx, _def_map, _hir, _typeck, diags) = typeck_source(
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
    let (_ctx, _def_map, _hir, _typeck, diags) = typeck_source(
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
    let (_ctx, _def_map, _hir, _typeck, diags) = typeck_source(
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
    let (_ctx, _def_map, _hir, _typeck, diags) = typeck_source(
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
    let (_ctx, _def_map, _hir, _typeck, diags) = typeck_source(
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
