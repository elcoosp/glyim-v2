use crate::LlvmBackend;
use glyim_codegen::CodegenBackend;
use glyim_core::Interner;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::Mutability;
use glyim_mir::{
    BasicBlockData, Body, LocalDecl, LocalIdx, SourceInfo, Terminator, TerminatorKind,
};
use glyim_span::Span;
use glyim_type::{TyCtxMut, TyKind};

/// Build a trivial body whose owner is `FnDefId(7)`. No `FnSig` is registered
/// for that def id, which must now surface as an internal compiler error from
/// `lower_body` (Tier 5.3) instead of silently lowering with an empty FnSig.
fn build_body_without_fn_sig(ctx: &mut TyCtxMut) -> Body {
    let i32_ty = ctx.mk_ty(TyKind::Int(glyim_core::primitives::IntTy::I32));

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl {
        ty: i32_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });

    let mut basic_blocks: IndexVec<glyim_mir::BasicBlockIdx, BasicBlockData> = IndexVec::new();
    let bb0 = BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    basic_blocks.push(bb0);

    Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(7)),
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty: i32_ty,
        span: Span::DUMMY,
        var_debug_info: Vec::new(),
    }
}

#[test]
fn t53_missing_fn_sig_is_internal_compiler_error() {
    let mut ctx = TyCtxMut::new(Interner::default());
    let body = build_body_without_fn_sig(&mut ctx);
    let frozen = ctx.freeze();

    // Deliberately do NOT register an FnSig for FnDefId(7).
    let backend = LlvmBackend::new().with_ty_ctx(frozen);
    let body = std::sync::Arc::new(body);
    let result = backend.generate_function(&body);

    assert!(
        result.is_err(),
        "lowering a body with no registered FnSig must error, got Ok"
    );
    let diags = result.unwrap_err();
    assert!(
        !diags.is_empty(),
        "the error must carry at least one diagnostic"
    );
    let msg = format!("{:?}", diags);
    assert!(
        msg.contains("no FnSig registered") || msg.contains("FnSig"),
        "diagnostic should explain the missing FnSig, got: {}",
        msg
    );
}
