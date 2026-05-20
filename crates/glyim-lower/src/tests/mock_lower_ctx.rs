use crate::lower::{AdtDef, AdtKind, AdtVariant, LowerCtx};
use glyim_core::def_id::{AdtId, ConstDefId, FnDefId};
use glyim_core::interner::Name;
use glyim_mir;
use glyim_span::Span;
use glyim_type::{FieldIdx, FnSig, Substitution, TyCtx};
use std::collections::HashMap;

type FieldKey = (u32, u32, Name);

pub struct TestLowerCtx<'a> {
    pub ty_ctx: &'a TyCtx,
    field_indices: HashMap<FieldKey, FieldIdx>,
}

impl<'a> TestLowerCtx<'a> {
    pub fn new(ty_ctx: &'a TyCtx) -> Self {
        Self {
            ty_ctx,
            field_indices: HashMap::new(),
        }
    }

    pub fn add_field_index(
        &mut self,
        adt_id: AdtId,
        variant_idx: u32,
        field_name: Name,
        field_idx: FieldIdx,
    ) {
        self.field_indices
            .insert((adt_id.to_raw(), variant_idx, field_name), field_idx);
    }
}

impl<'a> LowerCtx for TestLowerCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ty_ctx
    }

    fn adt_def(&self, _id: AdtId) -> AdtDef {
        AdtDef {
            variants: vec![AdtVariant { fields: vec![] }],
            kind: AdtKind::Struct,
        }
    }

    fn push_span(&self, _span: Span) {}
    fn pop_span(&self) {}

    fn field_index_by_name(&self, adt_id: AdtId, variant_idx: u32, name: Name) -> Option<FieldIdx> {
        self.field_indices
            .get(&(adt_id.to_raw(), variant_idx, name))
            .copied()
    }

    fn variant_index_by_name(&self, _adt_id: AdtId, _name: Name) -> Option<u32> {
        None
    }

    fn fn_sig(&self, _def_id: FnDefId) -> Option<FnSig> {
        None
    }

    fn const_value(
        &self,
        _def_id: ConstDefId,
        _substs: Substitution,
    ) -> Option<glyim_mir::MirConst> {
        None
    }
}
