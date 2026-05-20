use glyim_core::Name;
use glyim_core::def_id::{ConstDefId, FnDefId};
use glyim_diag::GlyimDiagnostic;
use glyim_span::Span;
use glyim_type::*;
use glyim_typeck::thir;

#[derive(Clone, Debug)]
pub struct LowerResult {
    pub body: glyim_mir::Body,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

/// Context trait provided by the caller to the THIR→MIR lowering.
///
/// Implementors provide type information, ADT definitions, and name-resolution
/// capabilities that the lowering needs but cannot access from THIR alone.
pub trait LowerCtx {
    /// Access the frozen type context.
    fn ty_ctx(&self) -> &TyCtx;

    /// Get the ADT definition for the given ADT ID.
    fn adt_def(&self, id: glyim_core::def_id::AdtId) -> AdtDef;

    /// Push a source span onto the span stack (for diagnostic context).
    fn push_span(&self, span: Span);

    /// Pop a source span from the span stack.
    fn pop_span(&self);

    /// Resolve a field by name within a specific variant of an ADT.
    ///
    /// Returns the `FieldIdx` of the field if found, or `None` if the field
    /// name is not present in the given variant.
    fn field_index_by_name(
        &self,
        _adt_id: glyim_core::def_id::AdtId,
        _variant_idx: u32,
        _name: Name,
    ) -> Option<FieldIdx> {
        None
    }

    /// Resolve a variant by name within an ADT.
    ///
    /// Returns the variant index if found, or `None` if no variant with
    /// that name exists.
    fn variant_index_by_name(
        &self,
        _adt_id: glyim_core::def_id::AdtId,
        _name: Name,
    ) -> Option<u32> {
        None
    }

    /// Get the function signature for a function definition.
    fn fn_sig(&self, _def_id: FnDefId) -> Option<FnSig> {
        None
    }

    /// Get the constant value for a constant definition.
    fn const_value(
        &self,
        _def_id: ConstDefId,
        _substs: Substitution,
    ) -> Option<glyim_mir::MirConst> {
        None
    }
}

/// ADT definition used during lowering.
#[derive(Clone, Debug)]
pub struct AdtDef {
    pub variants: Vec<AdtVariant>,
    pub kind: AdtKind,
}

/// A single variant of an ADT (struct field list, enum variant, or union field).
#[derive(Clone, Debug)]
pub struct AdtVariant {
    pub fields: Vec<Ty>,
}

/// The kind of ADT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdtKind {
    Struct,
    Enum,
    Union,
}

/// Lower a THIR body to MIR.
pub fn lower_body(ctx: &dyn LowerCtx, thir: &thir::Body) -> LowerResult {
    let mut builder = crate::builder::MirBuilder::new(ctx, thir);
    builder.lower_body(thir);

    let mut body = glyim_mir::Body::dummy(builder.owner);
    body.basic_blocks = builder.basic_blocks;
    body.locals = builder.locals;
    body.arg_count = builder.arg_count;
    body.return_ty = builder.return_ty;
    body.span = builder.span;

    LowerResult {
        body,
        diagnostics: builder.diagnostics,
    }
}
