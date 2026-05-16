use crate::lower::lower_crate;
use crate::{Expr, ExprId, ItemId, ItemKind};
use glyim_core::interner::Interner;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;

fn last_expr_id(body: &crate::Body) -> ExprId {
    ExprId::from_raw(body.exprs.len() as u32 - 1)
}
#[test]
fn test_hir_item_ids_are_unique() {
    let source = "fn foo() {} fn bar() {}";
    let parse_result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = crate::pipeline_api::lower_crate_for_pipeline(&parse_result.root, &mut interner);

    let ids: std::collections::HashSet<_> = hir.items.iter().map(|item| item.id).collect();
    assert_eq!(
        ids.len(),
        hir.items.len(),
        "All ItemIds should be unique, but found duplicates"
    );
}
#[test]
fn test_fn_item_with_params() {
    let source = "fn add(a: i32, b: i32) -> i32 { a + b }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let root = parse_result.root;
    let mut interner = Interner::new();
    let hir = lower_crate(&root, &mut interner);

    assert_eq!(hir.items.len(), 1, "should have one item");
    let item = &hir.items[ItemId::from_raw(0)];
    assert_eq!(interner.resolve(item.name), "add");
    match &item.kind {
        ItemKind::Fn(fn_item) => {
            assert_eq!(fn_item.params.len(), 2);
            assert_eq!(interner.resolve(fn_item.params[0].name), "a");
            assert_eq!(interner.resolve(fn_item.params[1].name), "b");
            assert!(fn_item.return_ty.is_some(), "should have return type");
            assert!(fn_item.body.is_some(), "should have body");
            assert!(!fn_item.is_unsafe);
            assert!(!fn_item.is_async);
            assert!(fn_item.generic_params.is_empty());

            let body_id = fn_item.body.unwrap();
            let body = &hir.bodies[body_id];
            assert!(!body.exprs.is_empty(), "body should have expressions");
            assert!(hir.body_owners.get(body_id).is_some());
            let block_id = last_expr_id(body);
            match &body.exprs[block_id] {
                Expr::Block { stmts, tail } => {
                    assert!(stmts.is_empty(), "should have no statements");
                    assert!(tail.is_some(), "should have tail expression (a+b)");
                }
                other => panic!("Expected Block expression, got {:?}", other),
            }
        }
        other => panic!("Expected Fn item, got {:?}", other),
    }
}

#[test]
fn test_fn_item_no_params() {
    let source = "fn foo() {}";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let root = parse_result.root;
    let mut interner = Interner::new();
    let hir = lower_crate(&root, &mut interner);

    assert_eq!(hir.items.len(), 1);
    let item = &hir.items[ItemId::from_raw(0)];
    match &item.kind {
        ItemKind::Fn(fn_item) => {
            assert!(fn_item.params.is_empty());
            assert!(fn_item.return_ty.is_none());
            assert!(fn_item.body.is_some());
            let body_id = fn_item.body.unwrap();
            let body = &hir.bodies[body_id];
            assert!(
                body.exprs.len() == 1,
                "body should have exactly one block expression"
            );
            let block_id = last_expr_id(body);
            match &body.exprs[block_id] {
                Expr::Block { stmts, tail } => {
                    assert!(stmts.is_empty());
                    assert!(tail.is_none());
                }
                other => panic!("Expected Block expression, got {:?}", other),
            }
        }
        other => panic!("Expected Fn item, got {:?}", other),
    }
}
