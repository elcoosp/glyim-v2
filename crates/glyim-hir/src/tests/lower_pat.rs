use crate::lower::lower_crate;
use crate::{BodyId, Expr, ExprId, ItemId, ItemKind};
use glyim_core::interner::Interner;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;

fn get_body_hir(source: &str) -> (crate::CrateHir, Interner, BodyId) {
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let hir = lower_crate(&parse_result.root, &mut interner, &mut Vec::new());
    let body_id = match &hir.items[ItemId::from_raw(0)].kind {
        ItemKind::Fn(fn_item) => fn_item.body.expect("no body"),
        other => panic!("expected Fn item, got {:?}", other),
    };
    (hir, interner, body_id)
}

fn get_body(hir: &crate::CrateHir, body_id: BodyId) -> &crate::Body {
    &hir.bodies[body_id]
}

#[test]
fn test_pat_binding() {
    let (hir, interner, body_id) = get_body_hir("fn f() { let x = 1; x }");
    let body = get_body(&hir, body_id);
    let block_id = ExprId::from_raw(body.exprs.len() as u32 - 1);
    match &body.exprs[block_id] {
        Expr::Block { stmts, .. } => {
            let found = stmts.iter().any(|&sid| {
                if let Expr::Assign { lhs, .. } = &body.exprs[sid]
                    && let Expr::Path(p) = &body.exprs[*lhs] {
                        return p.as_name().unwrap() == interner.intern("x");
                    }
                false
            });
            assert!(found, "No assignment to x found in block statements");
        }
        _ => panic!(),
    }
}

#[test]
fn test_pat_struct() {
    let src = "struct Point { x: i32, y: i32 }
               fn f(p: Point) { let Point { x, y } = p; }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(src, file_id);
    let mut interner = Interner::new();
    let hir = lower_crate(&parse_result.root, &mut interner, &mut Vec::new());
    let body_id = match &hir.items[ItemId::from_raw(1)].kind {
        ItemKind::Fn(fn_item) => fn_item.body.expect("no body"),
        other => panic!("expected Fn item, got {:?}", other),
    };
    let body = &hir.bodies[body_id];
    let block_id = ExprId::from_raw(body.exprs.len() as u32 - 1);
    match &body.exprs[block_id] {
        Expr::Block { stmts, .. } => {
            let found = stmts
                .iter()
                .any(|&sid| matches!(&body.exprs[sid], Expr::Assign { .. }));
            assert!(found, "Expected assignment inside block");
        }
        _ => panic!(),
    }
}
