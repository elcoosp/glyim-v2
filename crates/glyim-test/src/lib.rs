use glyim_core::primitives::Mutability;
use glyim_core::interner::Interner;
use glyim_type::{Ty, TyCtx, TyCtxMut, TyKind, TypeLookup, PrintTy};
use glyim_mir::{Body, SourceInfo};
use glyim_span::{Span};

pub struct TestCtxBuilder {
    interner: Option<Interner>,
}

impl TestCtxBuilder {
    pub fn new() -> Self { Self { interner: None } }
    pub fn with_interner(mut self, interner: Interner) -> Self {
        self.interner = Some(interner);
        self
    }
    pub fn build(self) -> TyCtxMut {
        let interner = self.interner.unwrap_or_default();
        TyCtxMut::new(interner)
    }
}

impl Default for TestCtxBuilder {
    fn default() -> Self { Self::new() }
}

pub fn test_ty_ctx() -> TyCtxMut { TestCtxBuilder::new().build() }
pub fn test_frozen_ty_ctx() -> TyCtx { test_ty_ctx().freeze() }

// Helper: assert type kind using generic TypeLookup
pub fn assert_ty_kind<L: TypeLookup>(ctx: &L, ty: Ty, expected: &TyKind) {
    let actual = ctx.ty_kind(ty);
    assert_eq!(actual, expected, "type mismatch: expected {:?}, got {}", expected, PrintTy::new(ty, ctx));
}

pub fn assert_is_error(ctx: &TyCtx, ty: Ty) {
    assert!(ctx.ty_is_error(ty), "expected error type, got {}", PrintTy::new(ty, ctx));
}

pub fn assert_is_int(ctx: &TyCtx, ty: Ty, expected: glyim_core::primitives::IntTy) {
    match ctx.ty_kind(ty) {
        TyKind::Int(i) if *i == expected => {}
        other => panic!("expected {:?}, got {} (actual: {:?})", expected, PrintTy::new(ty, ctx), other),
    }
}

pub fn dummy_mir_body() -> Body {
    use glyim_core::def_id::{DefId, CrateId, LocalDefId};
    let mut basic_blocks = glyim_core::arena::IndexVec::new();
    basic_blocks.push(glyim_mir::BasicBlockData::new(glyim_mir::Terminator {
        kind: glyim_mir::TerminatorKind::Unreachable,
        source_info: SourceInfo::new(Span::DUMMY),
    }));
    let mut locals = glyim_core::arena::IndexVec::new();
    locals.push(glyim_mir::LocalDecl {
        ty: Ty::ERROR,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty: Ty::ERROR,
        span: Span::DUMMY,
        var_debug_info: Vec::new(),
    }
}
