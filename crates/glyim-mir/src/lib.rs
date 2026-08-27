//! Mid-Level IR — CFG form.
//!
//! [F2] Uses `Ty::ERROR` instead of `Ty::from_raw(0)`.
//! [F9] `Place::ty()` matches on `&TyKind` and extracts `Copy`
//! fields (`Ty`, `Substitution`) without cloning the entire TyKind.

use glyim_core::arena::IndexVec;
use glyim_core::def_id::*;
use glyim_core::def_id::{ConstDefId, FnDefId};
use glyim_core::interner::Name;
use glyim_core::primitives::*;
use glyim_span::Span;
use glyim_type::*;

#[allow(missing_docs)]
#[allow(missing_docs)]
glyim_core::define_idx!(BasicBlockIdx);
#[allow(missing_docs)]
#[allow(missing_docs)]
glyim_core::define_idx!(LocalIdx);
#[allow(missing_docs)]
#[allow(missing_docs)]
glyim_core::define_idx!(VariantIdx);

#[derive(Clone, Debug)]
/// Body.
pub struct Body {
/// Struct.
    pub owner: DefId,
/// Struct.
    pub basic_blocks: IndexVec<BasicBlockIdx, BasicBlockData>,
/// Struct.
    pub locals: IndexVec<LocalIdx, LocalDecl>,
/// Struct.
    pub arg_count: usize,
/// Struct.
    pub return_ty: Ty,
/// Struct.
    pub span: Span,
/// Struct.
    pub var_debug_info: Vec<VarDebugInfo>,
}

#[derive(Clone, Debug)]
/// VarDebugInfo.
pub struct VarDebugInfo {
/// Struct.
    pub name: Name,
/// Struct.
    pub value: VarDebugInfoValue,
}

#[derive(Clone, Debug)]
/// VarDebugInfoValue.
pub enum VarDebugInfoValue {
#[allow(missing_docs)]
    Place(Place),
#[allow(missing_docs)]
    Const(MirConst),
}

#[derive(Clone, Debug)]
/// BasicBlockData.
pub struct BasicBlockData {
/// Struct.
    pub statements: Vec<Statement>,
/// Struct.
    pub terminator: Terminator,
/// Struct.
    pub is_cleanup: bool,
}

impl BasicBlockData {
/// new.
    pub fn new(terminator: Terminator) -> Self {
        Self {
            statements: Vec::new(),
            terminator,
            is_cleanup: false,
        }
    }
}

#[derive(Clone, Debug)]
/// Statement.
pub struct Statement {
/// Struct.
    pub kind: StatementKind,
/// Struct.
    pub source_info: SourceInfo,
}

#[derive(Clone, Debug)]
/// StatementKind.
pub enum StatementKind {
#[allow(missing_docs)]
    Assign(Place, Rvalue),
#[allow(missing_docs)]
    StorageLive(LocalIdx),
#[allow(missing_docs)]
    StorageDead(LocalIdx),
/// Variant.
    Nop,
}

#[derive(Clone, Debug)]
/// Rvalue.
pub enum Rvalue {
#[allow(missing_docs)]
    Use(Operand),
#[allow(missing_docs)]
    Ref(Place, BorrowKind),
#[allow(missing_docs)]
    BinaryOp(BinOp, Box<(Operand, Operand)>),
#[allow(missing_docs)]
    UnaryOp(UnOp, Operand),
#[allow(missing_docs)]
    Aggregate(AggregateKind, Vec<Operand>),
#[allow(missing_docs)]
    Discriminant(Place),
#[allow(missing_docs)]
    Len(Place),

    /// Dynamic call via vtable.
    Cast(CastKind, Operand, Ty),
#[allow(missing_docs)]
    Repeat(Operand, MirConst),
}

#[derive(Clone, Debug)]
/// AggregateKind.
pub enum AggregateKind {
#[allow(missing_docs)]
    Array(Ty),
/// Variant.
    Tuple,
#[allow(missing_docs)]
    Adt(AdtId, VariantIdx, Substitution),
#[allow(missing_docs)]
    Closure(ClosureId, Substitution),
}

#[derive(Clone, Debug)]
/// Operand.
pub enum Operand {
#[allow(missing_docs)]
    Copy(Place),
#[allow(missing_docs)]
    Move(Place),
#[allow(missing_docs)]
    Constant(MirConst),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// Place.
pub struct Place {
/// Struct.
    pub local: LocalIdx,
/// Struct.
    pub projection: Box<[ProjectionElem]>,
}

impl Place {
/// new.
    pub fn new(local: LocalIdx) -> Self {
        Self {
            local,
            projection: Box::new([]),
        }
    }

/// ty.
    pub fn ty(&self, ctx: &dyn TypeLookup, local_decls: &IndexVec<LocalIdx, LocalDecl>) -> Ty {
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
                    TyKind::Adt(adt_id, _substs) => ctx.field_ty(*adt_id, idx.to_raw() as usize),
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
                ProjectionElem::Downcast(_variant_idx) => {
                    // Downcast keeps the same ADT type; the variant's fields are accessed via Field projections.
                    // So we keep ty unchanged.
                    ty
                }
                ProjectionElem::ConstantIndex {
                    offset: _,
                    min_length: _,
                    from_end: _,
                } => {
                    // Constant index returns the element type of the array/slice.
                    match ctx.ty_kind(ty) {
                        TyKind::Array(inner_ty, _) | TyKind::Slice(inner_ty) => *inner_ty,
                        _ => {
                            tracing::error!("Place::ty(): ConstantIndex on non-array/slice type");
                            ctx.error_ty()
                        }
                    }
                }
                ProjectionElem::Subslice {
                    from: _,
                    to: _,
                    from_end: _,
                } => {
                    // A subslice projection yields a slice type `[T]`. If the
                    // base is already a slice, keep it unchanged (the common
                    // `[T]`/`&[T]` slicing case). If the base is an array
                    // `[T; N]`, the read-only `TypeLookup` cannot intern a new
                    // `[T; len]` type, so we fall back to the element type; the
                    // allocating `ty_mut` computes the precise `[T; len]` (plan
                    // §11.1).
                    match ctx.ty_kind(ty) {
                        TyKind::Slice(_) => ty,
                        TyKind::Array(inner, _) => *inner,
                        _ => {
                            tracing::error!("Place::ty(): Subslice on non-array/slice type");
                            ctx.error_ty()
                        }
                    }
                }
            };
        }

        ty
    }

    /// Intern-capable variant of [`Place::ty`].
    ///
    /// Like `ty`, but when the projection yields a *new* type that must be
    /// allocated in the type arena (currently only `ProjectionElem::Subslice`
    /// applied to a fixed-size array base `[T; N]`, which produces `[T; len]`),
    /// this method interns that type via the mutable `TyCtxMut`. The
    /// read-only `ty` cannot do this because `Ty` is an interned index and
    /// `TypeLookup` is immutable (see plan §11.1).
    ///
    /// For every other projection the behavior is identical to `ty`.
    pub fn ty_mut(
        &self,
        ctx: &mut TyCtxMut,
        local_decls: &IndexVec<LocalIdx, LocalDecl>,
    ) -> Ty {
        let mut ty = local_decls[self.local].ty;

        for elem in self.projection.iter() {
            ty = match elem {
                ProjectionElem::Deref => match ctx.ty_kind(ty) {
                    TyKind::Ref(_, inner_ty, _) => *inner_ty,
                    TyKind::RawPtr(inner_ty, _) => *inner_ty,
                    _ => {
                        tracing::error!("Place::ty_mut(): Deref on non-pointer type");
                        ctx.error_ty()
                    }
                },
                ProjectionElem::Field(idx) => match ctx.ty_kind(ty) {
                    TyKind::Tuple(substs) => {
                        let args = ctx.substitution_args(*substs);
                        if let Some(GenericArg::Ty(field_ty)) = args.get(idx.to_raw() as usize) {
                            *field_ty
                        } else {
                            tracing::error!("Place::ty_mut(): Field index out of bounds for tuple");
                            ctx.error_ty()
                        }
                    }
                    TyKind::Adt(adt_id, _substs) => ctx.field_ty(*adt_id, idx.to_raw() as usize),
                    _ => {
                        tracing::error!("Place::ty_mut(): Field projection on non-tuple/ADT type");
                        ctx.error_ty()
                    }
                },
                ProjectionElem::Index(_) => match ctx.ty_kind(ty) {
                    TyKind::Array(inner_ty, _) => *inner_ty,
                    TyKind::Slice(inner_ty) => *inner_ty,
                    _ => {
                        tracing::error!("Place::ty_mut(): Index on non-array/slice type");
                        ctx.error_ty()
                    }
                },
                ProjectionElem::Downcast(_variant_idx) => ty,
                ProjectionElem::ConstantIndex {
                    offset: _,
                    min_length: _,
                    from_end: _,
                } => match ctx.ty_kind(ty) {
                    TyKind::Array(inner_ty, _) | TyKind::Slice(inner_ty) => *inner_ty,
                    _ => {
                        tracing::error!("Place::ty_mut(): ConstantIndex on non-array/slice type");
                        ctx.error_ty()
                    }
                },
                ProjectionElem::Subslice {
                    from,
                    to,
                    from_end,
                } => match ctx.ty_kind(ty) {
                    TyKind::Slice(_) => ty,
                    TyKind::Array(inner, len_const) => {
                        // Subslice of a fixed-size array `[T; N]` yields
                        // `[T; len]` where `len` is the subslice's element count.
                        let n = match len_const.kind {
                            ConstKind::Uint(v) => v as u64,
                            ConstKind::Int(v) => v as u64,
                            _ => {
                                // Unknown array length: cannot compute the
                                // subslice length without interned constants;
                                // fall back to the element type (old behavior).
                                tracing::warn!(
                                    "Place::ty_mut(): Subslice on array with non-constant length"
                                );
                                return *inner;
                            }
                        };
                        let len = if *from_end {
                            // `to` is the number of trailing elements to drop.
                            let end = n.saturating_sub(*to);
                            end.saturating_sub(*from)
                        } else {
                            // `to` is the exclusive end index.
                            to.saturating_sub(*from)
                        };
                        let subslice_const = Const {
                            kind: ConstKind::Uint(len as u128),
                            ty: Ty::USIZE,
                        };
                        ctx.alloc_ty(TyKind::Array(*inner, subslice_const))
                    }
                    _ => {
                        tracing::error!("Place::ty_mut(): Subslice on non-array/slice type");
                        ctx.error_ty()
                    }
                },
            };
        }

        ty
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// ProjectionElem.
pub enum ProjectionElem {
/// Variant.
    Deref,
#[allow(missing_docs)]
    Field(FieldIdx),
#[allow(missing_docs)]
    Index(LocalIdx),
#[allow(missing_docs)]
    Downcast(VariantIdx),
    /// Fixed index into a slice/array, used by slice patterns.
    /// For arrays, offset is always from the start.
    /// For slices, from_end determines direction.
    ConstantIndex {
/// Struct.
        offset: u64,
/// Struct.
        min_length: u64,
/// Struct.
        from_end: bool,
    },
    /// Represents a subslice in a pattern: [prefix, .., suffix]
    Subslice {
/// Struct.
        from: u64,
/// Struct.
        to: u64,
/// Struct.
        from_end: bool,
    },
}

#[derive(Clone, Debug)]
/// LocalDecl.
pub struct LocalDecl {
/// Struct.
    pub ty: Ty,
/// Struct.
    pub mutability: Mutability,
/// Struct.
    pub source_info: SourceInfo,
}

#[derive(Clone, Debug)]
/// MirConst.
pub struct MirConst {
/// Struct.
    pub kind: MirConstKind,
/// Struct.
    pub ty: Ty,
/// Struct.
    pub span: Span,
}

#[derive(Clone, Debug)]
/// MirConstKind.
pub enum MirConstKind {
#[allow(missing_docs)]
    Int(i128),
#[allow(missing_docs)]
    Uint(u128),
#[allow(missing_docs)]
    FloatBits(u64),
#[allow(missing_docs)]
    Bool(bool),
#[allow(missing_docs)]
    Char(char),
#[allow(missing_docs)]
    String(Name),
/// Variant.
    Unit,
#[allow(missing_docs)]
    Fn(FnDefId, Substitution),
#[allow(missing_docs)]
    ConstRef(ConstDefId, Substitution),
    /// A trait-method reference that must be devirtualized at
    /// monomorphization (or resolved at interpretation) against the concrete
    /// type of the call's first argument (the receiver). Used for calls on a
    /// generic-bound receiver (`f.poll()` where `f: F` and `F: Future`): the
    /// concrete `impl` method `FnDefId` is not known until the generic is
    /// instantiated, so the method identity is carried here and resolved from
    /// the receiver's (monomorphized) type via `TyCtx::resolve_trait_method`.
    VirtualMethod {
        /// The trait the method belongs to.
        trait_def_id: TraitDefId,
        /// The method name within the trait.
        method_name: Name,
    },
    /// Variant.
    Aggregate(Vec<MirConst>),
/// Variant.
    Error,
}

#[derive(Clone, Debug)]
/// Terminator.
pub struct Terminator {
/// Struct.
    pub kind: TerminatorKind,
/// Struct.
    pub source_info: SourceInfo,
}

#[derive(Clone, Debug)]
/// TerminatorKind.
pub enum TerminatorKind {
/// Variant.
    Goto {
/// Struct.
        target: BasicBlockIdx,
    },
/// Variant.
    SwitchInt {
/// Struct.
        discr: Operand,
/// Struct.
        switch_ty: Ty,
/// Struct.
        targets: SwitchTargets,
    },
/// Variant.
    Return,
/// Variant.
    Unreachable,
/// Variant.
    Call {
/// Struct.
        func: Operand,
/// Struct.
        args: Vec<Operand>,
/// Struct.
        destination: Place,
/// Struct.
        target: Option<BasicBlockIdx>,
/// Struct.
        cleanup: Option<BasicBlockIdx>,
    },
/// Variant.
    Assert {
/// Struct.
        cond: Operand,
/// Struct.
        expected: bool,
/// Struct.
        target: BasicBlockIdx,
/// Struct.
        cleanup: Option<BasicBlockIdx>,
/// Struct.
        msg: AssertMessage,
    },
/// Variant.
    Drop {
/// Struct.
        place: Place,
/// Struct.
        target: BasicBlockIdx,
/// Struct.
        cleanup: Option<BasicBlockIdx>,
    },
}

#[derive(Clone, Debug)]
/// AssertMessage.
pub enum AssertMessage {
#[allow(missing_docs)]
    Overflow(BinOp),
/// Variant.
    DivisionByZero,
/// Variant.
    RemainderByZero,
/// Variant.
    BoundsCheck,
}

#[derive(Clone, Debug)]
/// SwitchTargets.
pub struct SwitchTargets {
    branches: Box<[(u128, BasicBlockIdx)]>,
    otherwise: BasicBlockIdx,
}

impl SwitchTargets {
/// new.
    pub fn new(branches: Box<[(u128, BasicBlockIdx)]>, otherwise: BasicBlockIdx) -> Self {
        Self {
            branches,
            otherwise,
        }
    }
/// otherwise.
    pub fn otherwise(&self) -> BasicBlockIdx {
        self.otherwise
    }
/// iter.
    pub fn iter(&self) -> impl Iterator<Item = (u128, BasicBlockIdx)> + '_ {
        self.branches.iter().copied()
    }
/// if_switch.
    pub fn if_switch(then_bb: BasicBlockIdx, else_bb: BasicBlockIdx) -> Self {
        Self {
            branches: Box::new([(1, then_bb)]),
            otherwise: else_bb,
        }
    }
}

#[derive(Clone, Debug)]
/// SourceInfo.
pub struct SourceInfo {
/// Struct.
    pub span: Span,
}

impl SourceInfo {
/// new.
    pub fn new(span: Span) -> Self {
        Self { span }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// BorrowKind.
pub enum BorrowKind {
/// Variant.
    Shared,
/// Variant.
    Unique,
/// Variant.
    Mut {
        /// allow_two_phase_borrow field.
        allow_two_phase_borrow: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// CastKind.
pub enum CastKind {
/// Variant.
    IntToInt,
/// Variant.
    FloatToInt,
/// Variant.
    IntToFloat,
/// Variant.
    FloatToFloat,
/// Variant.
    PtrToPtr,
/// Variant.
    FnPtrToPtr,
/// Variant.
    PtrToInt,
/// Variant.
    IntToPtr,
}

impl Body {
/// dummy.
    pub fn dummy(owner: DefId) -> Self {
        let mut basic_blocks = IndexVec::new();
        let _bb0 = basic_blocks.push(BasicBlockData::new(Terminator {
            kind: TerminatorKind::Unreachable,
            source_info: SourceInfo::new(Span::DUMMY),
        }));

        let mut locals = IndexVec::new();
        locals.push(LocalDecl {
            ty: Ty::UNIT,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        Self {
            owner,
            basic_blocks,
            locals,
            arg_count: 0,
            return_ty: Ty::UNIT,
            span: Span::DUMMY,
            var_debug_info: Vec::new(),
        }
    }

/// args.
    pub fn args(&self) -> &[LocalDecl] {
        &self.locals.as_slice()[1..1 + self.arg_count]
    }
/// return_place.
    pub fn return_place(&self) -> Place {
        Place::new(LocalIdx::from_raw(0))
    }
}

#[cfg(test)]
mod tests;
