//! Tests for §13.2: const-eval cast legality now consults the shared
//! `glyim_type::is_valid_cast` when a `TyCtx` is attached via `with_ty_ctx`.

use crate::{ConstEvaluator, ConstValue};
use glyim_core::interner::{Interner, Name};
use glyim_core::primitives::{IntTy, UintTy};
use glyim_hir::{Body, Expr, ExprId, Literal, Path, TypeRef};
use glyim_span::{ByteIdx, FileId, Span};
use glyim_type::TyCtxMut;

fn dummy_span() -> Span {
    Span::new(
        FileId::BOGUS,
        ByteIdx::ZERO,
        ByteIdx::ZERO,
        glyim_span::SyntaxContext::ROOT,
    )
}

fn interner() -> &'static Interner {
    static I: std::sync::OnceLock<Interner> = std::sync::OnceLock::new();
    I.get_or_init(Interner::new)
}

fn test_body() -> Body {
    Body {
        owner: glyim_core::def_id::LocalDefId::from_raw(0),
        exprs: glyim_core::arena::IndexVec::new(),
        pats: glyim_core::arena::IndexVec::new(),
        params: Vec::new(),
        span: dummy_span(),
        expr_spans: glyim_core::arena::IndexVec::new(),
    }
}

fn alloc_ty_path(body: &mut Body, name: &str) -> ExprId {
    let n: Name = interner().intern(name);
    let ty = TypeRef::Path(Path::from_single(n));
    let lit = body.alloc_expr(Expr::Literal(Literal::Int(7, Some(IntTy::I32))), dummy_span());
    body.alloc_expr(Expr::Cast { expr: lit, ty }, dummy_span())
}

#[test]
fn legal_cast_succeeds_with_ty_ctx_gate() {
    // Plan §13.2: `i32 as u8` is a legal cast; with a `TyCtx` attached the
    // gate (`is_valid_cast`) permits it and the primitive converter produces
    // `Uint(7, U8)`.
    let mut tcx_mut = TyCtxMut::new(Interner::new());
    let mut body = test_body();
    let cast_id = alloc_ty_path(&mut body, "u8");
    let mut evaluator = ConstEvaluator::new(&body)
        .with_interner(interner())
        .with_ty_ctx(&mut tcx_mut);
    let result = evaluator.evaluate(cast_id).expect("legal cast must succeed");
    assert_eq!(result, ConstValue::Uint(7, UintTy::U8));
}

#[test]
fn legal_cast_succeeds_without_ty_ctx() {
    // Backward compatibility: without a `TyCtx` the gate is skipped and the
    // primitive converter still performs the cast (pre-§13.2 behavior).
    let mut body = test_body();
    let cast_id = alloc_ty_path(&mut body, "u8");
    let mut evaluator = ConstEvaluator::new(&body).with_interner(interner());
    let result = evaluator.evaluate(cast_id).expect("legal cast must succeed");
    assert_eq!(result, ConstValue::Uint(7, UintTy::U8));
}
