use crate::{AdtDef, AdtKind, AdtVariant, LowerCtx};
use glyim_core::def_id::{AdtId, ConstDefId, FnDefId};
use glyim_core::interner::Name;
use glyim_mir;
use glyim_span::Span;
use glyim_type::{FieldIdx, FnSig, Substitution, TyCtx};
use std::cell::RefCell;
use std::collections::HashMap;

type FieldKey = (u32, u32, Name);

pub struct MockLowerCtx<'a> {
    ty_ctx: &'a TyCtx,
    span_stack: RefCell<Vec<Span>>,
    field_indices: HashMap<FieldKey, FieldIdx>,
    adt_defs: HashMap<u32, AdtDef>,
}

impl<'a> MockLowerCtx<'a> {
    pub fn new(ty_ctx: &'a TyCtx) -> Self {
        Self {
            ty_ctx,
            span_stack: RefCell::new(Vec::new()),
            field_indices: HashMap::new(),
            adt_defs: HashMap::new(),
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
            variants: vec![AdtVariant { fields: vec![] }],
            kind: AdtKind::Struct,
        })
    }

    fn push_span(&self, span: Span) {
        self.span_stack.borrow_mut().push(span);
    }

    fn pop_span(&self) {
        self.span_stack.borrow_mut().pop();
    }

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
