//! Mock implementation of LowerCtx for testing.
use glyim_core::def_id::{AdtId, ConstDefId, FnDefId};
use glyim_core::interner::Name;
use glyim_lower::{AdtDef, AdtKind, LowerCtx};
use glyim_mir;
use glyim_span::Span;
use glyim_type::{FnSig, FieldIdx, Substitution, TyCtx};
use std::cell::RefCell;
use std::collections::HashMap;

/// Key for field index lookups: (AdtId raw, variant_idx, Name)
type FieldKey = (u32, u32, Name);

/// Key for variant index lookups: (AdtId raw, Name)
type VariantKey = (u32, Name);

pub struct MockLowerCtx<'a> {
    ty_ctx: &'a TyCtx,
    span_stack: RefCell<Vec<Span>>,
    /// Map from (AdtId raw, variant_idx, field_name) to field index
    field_indices: HashMap<FieldKey, FieldIdx>,
    /// Map from (AdtId raw, variant_name) to variant index
    variant_indices: HashMap<VariantKey, u32>,
    /// ADT definitions keyed by AdtId raw
    adt_defs: HashMap<u32, AdtDef>,
}

/// Operations for span testing.
pub enum SpanOp {
    Push(Span),
    Pop,
}

impl<'a> MockLowerCtx<'a> {
    pub fn new(ty_ctx: &'a TyCtx) -> Self {
        Self {
            ty_ctx,
            span_stack: RefCell::new(Vec::new()),
            field_indices: HashMap::new(),
            variant_indices: HashMap::new(),
            adt_defs: HashMap::new(),
        }
    }

    /// Register a field index for an ADT variant.
    pub fn add_field_index(
        &mut self,
        adt_id: AdtId,
        variant_idx: u32,
        field_name: Name,
        field_idx: FieldIdx,
    ) {
        self.field_indices.insert((adt_id.to_raw(), variant_idx, field_name), field_idx);
    }

    /// Register a variant index for an ADT.
    pub fn add_variant_index(
        &mut self,
        adt_id: AdtId,
        variant_name: Name,
        variant_idx: u32,
    ) {
        self.variant_indices.insert((adt_id.to_raw(), variant_name), variant_idx);
    }

    /// Register an ADT definition.
    pub fn add_adt_def(&mut self, adt_id: AdtId, def: AdtDef) {
        self.adt_defs.insert(adt_id.to_raw(), def);
    }
}

impl<'a> LowerCtx for MockLowerCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ty_ctx
    }

    fn adt_def(&self, id: AdtId) -> AdtDef {
        self.adt_defs.get(&id.to_raw()).cloned().unwrap_or(AdtDef {
            variants: vec![],
            kind: AdtKind::Struct,
        })
    }

    fn push_span(&self, span: Span) {
        self.span_stack.borrow_mut().push(span);
    }

    fn pop_span(&self) {
        self.span_stack.borrow_mut().pop();
    }

    fn field_index_by_name(
        &self,
        adt_id: AdtId,
        variant_idx: u32,
        name: Name,
    ) -> Option<FieldIdx> {
        self.field_indices.get(&(adt_id.to_raw(), variant_idx, name)).copied()
    }

    fn variant_index_by_name(
        &self,
        adt_id: AdtId,
        name: Name,
    ) -> Option<u32> {
        self.variant_indices.get(&(adt_id.to_raw(), name)).copied()
    }

    fn fn_sig(&self, _def_id: FnDefId) -> Option<FnSig> {
        None
    }

    fn const_value(&self, _def_id: ConstDefId, _substs: Substitution) -> Option<glyim_mir::MirConst> {
        None
    }
}
