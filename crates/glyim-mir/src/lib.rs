//! Mid-Level IR — CFG form.
//!
//! [F2] Uses `Ty::ERROR` instead of `Ty::from_raw(0)`.
//! [F9] `Place::ty()` matches on `&TyKind` and extracts `Copy`
//! fields (`Ty`, `Substitution`) without cloning the entire TyKind.

use glyim_core::arena::IndexVec;
use glyim_core::def_id::*;
use glyim_core::interner::Name;
use glyim_core::primitives::*;
use glyim_span::Span;
use glyim_type::*;

glyim_core::define_idx!(BasicBlockIdx);
glyim_core::define_idx!(LocalIdx);
glyim_core::define_idx!(VariantIdx);

#[derive(Clone, Debug)]
pub struct Body {
    pub owner: DefId,
    pub basic_blocks: IndexVec<BasicBlockIdx, BasicBlockData>,
    pub locals: IndexVec<LocalIdx, LocalDecl>,
    pub arg_count: usize,
    pub return_ty: Ty,
    pub span: Span,
    pub var_debug_info: Vec<VarDebugInfo>,
}

#[derive(Clone, Debug)]
pub struct VarDebugInfo {
    pub name: Name,
    pub value: VarDebugInfoValue,
}

#[derive(Clone, Debug)]
pub enum VarDebugInfoValue {
    Place(Place),
    Const(MirConst),
}

#[derive(Clone, Debug)]
pub struct BasicBlockData {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
    pub is_cleanup: bool,
}

impl BasicBlockData {
    pub fn new(terminator: Terminator) -> Self {
        Self {
            statements: Vec::new(),
            terminator,
            is_cleanup: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Statement {
    pub kind: StatementKind,
    pub source_info: SourceInfo,
}

#[derive(Clone, Debug)]
pub enum StatementKind {
    Assign(Place, Rvalue),
    StorageLive(LocalIdx),
    StorageDead(LocalIdx),
    Nop,
}

#[derive(Clone, Debug)]
pub enum Rvalue {
    Use(Operand),
    Ref(Place, BorrowKind),
    BinaryOp(BinOp, Box<(Operand, Operand)>),
    UnaryOp(UnOp, Operand),
    Aggregate(AggregateKind, Vec<Operand>),
    Discriminant(Place),
    Len(Place),
    Cast(CastKind, Operand, Ty),
    Repeat(Operand, MirConst),
}

#[derive(Clone, Debug)]
pub enum AggregateKind {
    Array(Ty),
    Tuple,
    Adt(AdtId, VariantIdx, Substitution),
    Closure(ClosureId, Substitution),
}

#[derive(Clone, Debug)]
pub enum Operand {
    Copy(Place),
    Move(Place),
    Constant(MirConst),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    pub local: LocalIdx,
    pub projection: Box<[ProjectionElem]>,
}

impl Place {
    pub fn new(local: LocalIdx) -> Self {
        Self {
            local,
            projection: Box::new([]),
        }
    }

    /// [F9] Compute the type of this Place by walking the projection chain.
    pub fn ty(&self, ctx: &impl TypeLookup, local_decls: &IndexVec<LocalIdx, LocalDecl>) -> Ty {
        let mut ty = local_decls[self.local].ty;

        for elem in self.projection.iter() {
            ty = match elem {
                ProjectionElem::Deref => match ctx.ty_kind(ty) {
                    TyKind::Ref(_, inner_ty, _) => *inner_ty,
                    TyKind::RawPtr(inner_ty, _) => *inner_ty,
                    _ => {
                        tracing::error!("Place::ty(): Deref on non-pointer type");
                        ctx.error_ty()
                    }
                },
                ProjectionElem::Field(idx) => match ctx.ty_kind(ty) {
                    TyKind::Tuple(substs) => {
                        let args = ctx.substitution_args(*substs);
                        if let Some(GenericArg::Ty(field_ty)) = args.get(idx.to_raw() as usize) {
                            *field_ty
                        } else {
                            tracing::error!("Place::ty(): Field index out of bounds for tuple");
                            ctx.error_ty()
                        }
                    }
                    TyKind::Adt(_adt_id, _substs) => {
                        tracing::warn!("STUB: ADT field type lookup not yet implemented");
                        ty
                    }
                    _ => {
                        tracing::error!("Place::ty(): Field projection on non-tuple/ADT type");
                        ctx.error_ty()
                    }
                },
                ProjectionElem::Index(_) => match ctx.ty_kind(ty) {
                    TyKind::Array(inner_ty, _) => *inner_ty,
                    TyKind::Slice(inner_ty) => *inner_ty,
                    _ => {
                        tracing::error!("Place::ty(): Index on non-array/slice type");
                        ctx.error_ty()
                    }
                },
                ProjectionElem::Downcast(_) => ty,
            };
        }
        ty
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectionElem {
    Deref,
    Field(FieldIdx),
    Index(LocalIdx),
    Downcast(VariantIdx),
}

#[derive(Clone, Debug)]
pub struct LocalDecl {
    pub ty: Ty,
    pub mutability: Mutability,
    pub source_info: SourceInfo,
}

#[derive(Clone, Debug)]
pub struct MirConst {
    pub kind: MirConstKind,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum MirConstKind {
    Int(i128),
    Uint(u128),
    FloatBits(u64),
    Bool(bool),
    Char(char),
    String(Name),
    Unit,
    Error,
}

#[derive(Clone, Debug)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub source_info: SourceInfo,
}

#[derive(Clone, Debug)]
pub enum TerminatorKind {
    Goto {
        target: BasicBlockIdx,
    },
    SwitchInt {
        discr: Operand,
        switch_ty: Ty,
        targets: SwitchTargets,
    },
    Return,
    Unreachable,
    Call {
        func: Operand,
        args: Vec<Operand>,
        destination: Place,
        target: Option<BasicBlockIdx>,
        cleanup: Option<BasicBlockIdx>,
    },
    Assert {
        cond: Operand,
        expected: bool,
        target: BasicBlockIdx,
        cleanup: Option<BasicBlockIdx>,
        msg: AssertMessage,
    },
    Drop {
        place: Place,
        target: BasicBlockIdx,
        cleanup: Option<BasicBlockIdx>,
    },
}

#[derive(Clone, Debug)]
pub enum AssertMessage {
    Overflow(BinOp),
    DivisionByZero,
    RemainderByZero,
    BoundsCheck,
}

#[derive(Clone, Debug)]
pub struct SwitchTargets {
    branches: Box<[(u128, BasicBlockIdx)]>,
    otherwise: BasicBlockIdx,
}

impl SwitchTargets {
    pub fn new(branches: Box<[(u128, BasicBlockIdx)]>, otherwise: BasicBlockIdx) -> Self {
        Self {
            branches,
            otherwise,
        }
    }
    pub fn otherwise(&self) -> BasicBlockIdx {
        self.otherwise
    }
    pub fn iter(&self) -> impl Iterator<Item = (u128, BasicBlockIdx)> + '_ {
        self.branches.iter().copied()
    }
    pub fn if_switch(then_bb: BasicBlockIdx, else_bb: BasicBlockIdx) -> Self {
        Self {
            branches: Box::new([(1, then_bb)]),
            otherwise: else_bb,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SourceInfo {
    pub span: Span,
}

impl SourceInfo {
    pub fn new(span: Span) -> Self {
        Self { span }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BorrowKind {
    Shared,
    Unique,
    Mut { allow_two_phase_borrow: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastKind {
    IntToInt,
    FloatToInt,
    IntToFloat,
    PtrToPtr,
    FnPtrToPtr,
}

impl Body {
    /// [F2] Uses `Ty::ERROR` instead of `Ty::from_raw(0)`.
    pub fn dummy(owner: DefId) -> Self {
        let mut basic_blocks = IndexVec::new();
        let _bb0 = basic_blocks.push(BasicBlockData::new(Terminator {
            kind: TerminatorKind::Unreachable,
            source_info: SourceInfo::new(Span::DUMMY),
        }));

        let mut locals = IndexVec::new();
        locals.push(LocalDecl {
            ty: Ty::ERROR,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        Self {
            owner,
            basic_blocks,
            locals,
            arg_count: 0,
            return_ty: Ty::ERROR,
            span: Span::DUMMY,
            var_debug_info: Vec::new(),
        }
    }

    pub fn args(&self) -> &[LocalDecl] {
        &self.locals.as_slice()[1..1 + self.arg_count]
    }
    pub fn return_place(&self) -> Place {
        Place::new(LocalIdx::from_raw(0))
    }
}

#[cfg(test)]
mod tests;
