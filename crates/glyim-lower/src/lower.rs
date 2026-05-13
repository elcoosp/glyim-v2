// THIR → MIR structural conversion.

use glyim_core::arena::IndexVec;
use glyim_core::def_id::AdtId;
use glyim_core::primitives::Mutability;
use glyim_diag::GlyimDiagnostic;
use glyim_mir;
use glyim_span::Span;
use glyim_type::*;
use glyim_typeck::thir;

#[derive(Clone, Debug)]
pub struct LowerResult {
    pub body: glyim_mir::Body,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

pub trait LowerCtx {
    fn ty_ctx(&self) -> &TyCtx;
    fn adt_def(&self, id: AdtId) -> AdtDef;
    fn push_span(&self, span: Span);
    fn pop_span(&self);
}

pub struct AdtDef {
    pub variants: Vec<AdtVariant>,
    pub kind: AdtKind,
}

pub struct AdtVariant {
    pub fields: Vec<Ty>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdtKind {
    Struct,
    Enum,
    Union,
}

#[allow(dead_code)]
struct MirBuilder<'a> {
    _ctx: &'a dyn LowerCtx,
    _locals: IndexVec<glyim_mir::LocalIdx, glyim_mir::LocalDecl>,
    _basic_blocks: IndexVec<glyim_mir::BasicBlockIdx, glyim_mir::BasicBlockData>,
    _arg_count: usize,
    _return_ty: Ty,
    _owner: glyim_core::def_id::DefId,
    _span: Span,
    diagnostics: Vec<GlyimDiagnostic>,
}

impl<'a> MirBuilder<'a> {
    fn new(ctx: &'a dyn LowerCtx, thir: &thir::Body) -> Self {
        let mut locals = IndexVec::new();
        locals.push(glyim_mir::LocalDecl {
            ty: thir.return_ty,
            mutability: Mutability::Not,
            source_info: glyim_mir::SourceInfo::new(thir.span),
        });
        Self {
            _ctx: ctx,
            _locals: locals,
            _basic_blocks: IndexVec::new(),
            _arg_count: thir.params.len(),
            _return_ty: thir.return_ty,
            _owner: thir.owner,
            _span: thir.span,
            diagnostics: Vec::new(),
        }
    }

    #[allow(dead_code)]
    fn _alloc_local(&mut self, ty: Ty, mutability: Mutability, span: Span) -> glyim_mir::LocalIdx {
        self._locals.push(glyim_mir::LocalDecl {
            ty,
            mutability,
            source_info: glyim_mir::SourceInfo::new(span),
        })
    }
}

pub fn lower_body(ctx: &dyn LowerCtx, thir: &thir::Body) -> LowerResult {
    let builder = MirBuilder::new(ctx, thir);
    let body = glyim_mir::Body::dummy(builder._owner);
    LowerResult {
        body,
        diagnostics: builder.diagnostics,
    }
}
