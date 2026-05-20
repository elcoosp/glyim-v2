use crate::pipeline_api::lower_crate_for_pipeline;
use crate::{BodyId, CrateHir, Expr, ExprId, ItemKind, Pat};
use glyim_core::interner::Interner;
use glyim_test::phase::FrontendTester;

struct TestContext {
    hir: CrateHir,
    interner: Interner,
}

fn lower_source(source: &str) -> TestContext {
    let trace = FrontendTester::new(source).run();
    let root = trace.parse_tree.unwrap();
    let mut interner = Interner::new();
    let (hir, diags) = lower_crate_for_pipeline(&root, &mut interner);
    assert!(
        diags.is_empty(),
        "Lowering produced diagnostics: {:?}",
        diags
    );
    TestContext { hir, interner }
}

fn find_fn_body(ctx: &TestContext, fn_name: &str) -> Option<BodyId> {
    let fn_name_name = ctx.interner.intern(fn_name);
    for item in ctx.hir.items.iter() {
        if let ItemKind::Fn(fn_item) = &item.kind {
            if item.name == fn_name_name {
                return fn_item.body;
            }
        }
    }
    None
}

fn find_expr_in_body(
    hir: &CrateHir,
    body_id: BodyId,
    pred: impl Fn(&Expr) -> bool,
) -> Option<ExprId> {
    let body = &hir.bodies[body_id];
    for (id, expr) in body.exprs.iter_enumerated() {
        if pred(expr) {
            return Some(id);
        }
    }
    None
}

#[test]
fn test_for_loop_lowering() {
    let ctx = lower_source("fn test() { for i in 0..10 { } }");
    let body_id = find_fn_body(&ctx, "test").expect("Function test not found");
    let for_expr_id = find_expr_in_body(&ctx.hir, body_id, |e| matches!(e, Expr::For { .. }));
    assert!(for_expr_id.is_some(), "For expression not found");

    let body = &ctx.hir.bodies[body_id];
    if let Expr::For {
        pat,
        iterable,
        body: body_expr,
    } = &body.exprs[for_expr_id.unwrap()]
    {
        let pat_node = &body.pats[*pat];
        let i_name = ctx.interner.intern("i");
        assert!(matches!(pat_node, Pat::Binding { name, .. } if *name == i_name));
        assert!(matches!(&body.exprs[*iterable], Expr::Range { .. }));
        assert!(matches!(&body.exprs[*body_expr], Expr::Block { .. }));
    } else {
        panic!("Expr is not For");
    }
}

#[test]
fn test_range_expression_lowering() {
    let ctx = lower_source("fn test() { let _ = 1..=5; }");
    let body_id = find_fn_body(&ctx, "test").expect("Function test not found");
    let range_expr_id = find_expr_in_body(&ctx.hir, body_id, |e| matches!(e, Expr::Range { .. }));
    assert!(range_expr_id.is_some(), "Range expression not found");

    let body = &ctx.hir.bodies[body_id];
    if let Expr::Range {
        start,
        end,
        inclusive,
    } = &body.exprs[range_expr_id.unwrap()]
    {
        assert!(start.is_some(), "Range start missing");
        assert!(end.is_some(), "Range end missing");
        assert!(*inclusive, "Range should be inclusive");
    } else {
        panic!("Expr is not Range");
    }
}

#[test]
fn test_struct_expr_lowering() {
    let source = r#"
        struct Point { x: i32, y: i32 }
        fn test() { let p = Point { x: 10, y: 20 }; }
    "#;
    let ctx = lower_source(source);
    let body_id = find_fn_body(&ctx, "test").expect("Function test not found");
    let struct_expr_id = find_expr_in_body(&ctx.hir, body_id, |e| matches!(e, Expr::Struct { .. }));
    assert!(struct_expr_id.is_some(), "Struct expression not found");

    let body = &ctx.hir.bodies[body_id];
    if let Expr::Struct {
        path,
        fields,
        spread,
    } = &body.exprs[struct_expr_id.unwrap()]
    {
        let name = path
            .as_name()
            .expect("Struct path should be single segment");
        let point_name = ctx.interner.intern("Point");
        assert_eq!(name, point_name);
        assert_eq!(fields.len(), 2, "Expected 2 fields");
        assert!(spread.is_none(), "Spread should be None");
        let x_name = ctx.interner.intern("x");
        let y_name = ctx.interner.intern("y");
        assert_eq!(fields[0].0, x_name);
        assert_eq!(fields[1].0, y_name);
    } else {
        panic!("Expr is not Struct");
    }
}
