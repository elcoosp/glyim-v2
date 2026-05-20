//! Tests for ABI-aware argument passing (S08-T04)

use crate::BytecodeBackend;

#[test]
fn backend_instantiates_with_layout_provider() {
    let backend = BytecodeBackend::new();
    assert_eq!(backend.name(), "bytecode");
}

#[test]
fn backend_accepts_custom_layout_provider() {
    struct TestProvider;
    impl crate::LayoutProvider for TestProvider {
        fn field_offset(&self, _ty: glyim_type::Ty, _field_idx: glyim_type::FieldIdx) -> u64 {
            16
        }
    }
    let backend = BytecodeBackend::new()
        .with_layout_provider(Box::new(TestProvider));
    assert_eq!(backend.name(), "bytecode");
}
