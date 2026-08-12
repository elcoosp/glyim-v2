//! Tests for ABI-aware argument passing (S08-T04)

use crate::{BytecodeBackend, CodegenBackend, LayoutProvider};
use glyim_mir::VariantIdx;
use glyim_type::{FieldIdx, Ty};

struct TestProvider;

impl LayoutProvider for TestProvider {
    fn field_offset(&self, _ty: Ty, _field_idx: FieldIdx) -> u64 {
        16
    }
    fn size_of(&self, _ty: Ty) -> u64 {
        16
    }
    fn variant_type(&self, _enum_ty: Ty, _variant_idx: VariantIdx) -> Ty {
        Ty::ERROR
    }
}

#[test]
fn backend_instantiates_with_layout_provider() {
    let backend = BytecodeBackend::with_ty_ctx(std::sync::Arc::new(glyim_type::TyCtxMut::new(glyim_core::Interner::default()).freeze()), glyim_core::TargetInfo::default());
    assert_eq!(backend.name(), "bytecode");
}

#[test]
fn backend_accepts_custom_layout_provider() {
    let backend = BytecodeBackend::with_ty_ctx(std::sync::Arc::new(glyim_type::TyCtxMut::new(glyim_core::Interner::default()).freeze()), glyim_core::TargetInfo::default()).with_layout_provider(Box::new(TestProvider));
    assert_eq!(backend.name(), "bytecode");
}
