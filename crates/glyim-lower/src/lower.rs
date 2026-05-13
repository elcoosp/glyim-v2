// THIR → MIR structural conversion.

use glyim_core::arena::IndexVec;
use glyim_core::def_id::AdtId;
use glyim_core::primitives::Mutability;
use glyim_mir;
use glyim_typeck::thir;
use glyim_type::*;
use glyim_span::Span;
use glyim_diag::GlyimDiagnostic;

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
pub enum AdtKind { Struct, Enum, Union }

struct MirBuilder<'a> {
    ctx: &'a dyn LowerCtx,
    locals: IndexVec<glyim_mir::LocalIdx, glyim_mir::LocalDecl>,
    basic_blocks: IndexVec<glyim_mir::BasicBlockIdx, glyim_mir::BasicBlockData>,
    arg_count: usize,
    return_ty: Ty,
    owner: glyim_core::def_id::DefId,
    span: Span,
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
            ctx, locals,
            basic_blocks: IndexVec::new(),
            arg_count: thir.params.len(),
            return_ty: thir.return_ty,
            owner: thir.owner,
            span: thir.span,
            diagnostics: Vec::new(),
        }
    }

    fn alloc_local(&mut self, ty: Ty, mutability: Mutability, span: Span) -> glyim_mir::LocalIdx {
        self.locals.push(glyim_mir::LocalDecl {
            ty,
            mutability,
            source_info: glyim_mir::SourceInfo::new(span),
        })
    }
}

pub fn lower_body(ctx: &dyn LowerCtx, thir: &thir::Body) -> LowerResult {
    let builder = MirBuilder::new(ctx, thir);
    // STUB: actual lowering logic would go here
    let body = glyim_mir::Body::dummy(builder.owner);
    LowerResult { body, diagnostics: builder.diagnostics }
}
