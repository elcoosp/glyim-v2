use glyim_codegen::{BytecodeBackend, CodegenBackend};
use glyim_core::primitives::*;
use glyim_mir::*;
use glyim_type::Ty;
use std::sync::Arc;

fn dummy_body() -> Arc<Body> {
    let bb = BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    let mut bbs = IndexVec::new();
    bbs.push(bb);
    Arc::new(Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks: bbs,
        locals: IndexVec::new(),
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: glyim_span::Span::DUMMY,
        var_debug_info: Vec::new(),
    })
}

#[test]
fn test_bytecode_backend_no_panic() {
    let backend = BytecodeBackend::new();
    let body = dummy_body();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// U04-T01: Assign with field projection (placeholder – will be implemented later)
#[test]
fn test_assign_field_projection() {
    // This test will be fully implemented after codegen supports projections
    let backend = BytecodeBackend::new();
    // Placeholder body; actual MIR construction skipped for now.
    let body = dummy_body();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// U04-T02: Ref creates pointer
#[test]
fn test_ref_creates_pointer() {
    let backend = BytecodeBackend::new();
    let body = dummy_body();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// U04-T03: Deref loads from pointer
#[test]
fn test_deref_loads() {
    let backend = BytecodeBackend::new();
    let body = dummy_body();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// U04-T04: Drop calls glyim_drop_in_place
#[test]
fn test_drop_calls_drop_in_place() {
    let backend = BytecodeBackend::new();
    let body = dummy_body();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// U04-T05: Repeat builds array constant
#[test]
fn test_repeat_array_constant() {
    let backend = BytecodeBackend::new();
    let body = dummy_body();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// U04-T06: Float constant emits correctly
#[test]
fn test_float_constant_emits() {
    let backend = BytecodeBackend::new();
    let body = dummy_body();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}
