use crate::thir;
use glyim_core::def_id::LocalDefId;
use glyim_core::interner::Name;
use glyim_core::primitives::*;
use glyim_hir::*;
use glyim_solve::InferenceTable;
use glyim_type::*;

/// Helper: create a minimal CrateHir with one function body containing the given expressions.
/// Returns the CrateHir and the BodyId of that function.
pub fn make_single_body_hir(exprs: Vec<Expr>) -> (CrateHir, glyim_hir::BodyId) {
    let mut hir = CrateHir {
        items: glyim_core::arena::IndexVec::new(),
        bodies: glyim_core::arena::IndexVec::new(),
        body_owners: glyim_core::arena::IndexVec::new(),
    };

    // Create body with given expressions
    let mut body_exprs = glyim_core::arena::IndexVec::new();
    for expr in exprs {
        body_exprs.push(expr);
    }
    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs: body_exprs,
        pats: glyim_core::arena::IndexVec::new(),
        params: Vec::new(),
        span: glyim_span::Span::DUMMY,
    };
    let body_id = hir.bodies.push(body);
    hir.body_owners.push(LocalDefId::from_raw(0));

    // Create a function item that owns this body
    let fn_item = Item {
        id: ItemId::from_raw(0),
        name: name("test_fn"),
        kind: ItemKind::Fn(FnItem {
            params: Vec::new(),
            return_ty: None,
            body: Some(body_id),
            is_unsafe: false,
            is_async: false,
            generic_params: Vec::new(),
            where_clauses: Vec::new(),
        }),
        visibility: Visibility::Public,
        span: glyim_span::Span::DUMMY,
    };
    hir.items.push(fn_item);

    (hir, body_id)
}

/// Run check_function_body with a fresh context and return the THIR body.
pub fn typeck_single_body(hir: &CrateHir, body_id: BodyId) -> thir::Body {
    let interner = glyim_core::interner::Interner::new();
    let mut ctx = TyCtxMut::new(interner);
    let mut infer = InferenceTable::new();
    let mut diagnostics = Vec::new();
    let mut obligations = Vec::new();

    crate::check_body::check_function_body(
        &mut ctx,
        &mut infer,
        &mut diagnostics,
        &mut obligations,
        body_id,
        hir,
        LocalDefId::from_raw(0),
        Ty::UNIT,
        &[],
    )
}

/// Create a simple Name from a static string.
pub fn name(s: &str) -> Name {
    let interner = glyim_core::interner::Interner::new();
    interner.intern(s)
}
