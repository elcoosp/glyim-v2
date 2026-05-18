use crate::{BytecodeBackend, CodegenBackend};
use glyim_core::{
    IndexVec,
    def_id::{CrateId, DefId, LocalDefId},
    primitives::*,
};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{FieldIdx, Ty};
use std::sync::Arc;

fn dummy_body() -> Arc<Body> {
    let bb = BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    let mut bbs = IndexVec::new();
    bbs.push(bb);
    Arc::new(Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks: bbs,
        locals: IndexVec::new(),
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: Vec::new(),
    })
}

fn check_opcode(bytecode: &[u8], expected_op: u8) -> bool {
    bytecode.iter().any(|&b| b == expected_op)
}

// U04-T01: Assign with field projection
#[test]
fn test_assign_field_projection() {
    let backend = BytecodeBackend::new();
    let local = LocalIdx::from_raw(0);
    let field_idx = FieldIdx::from_raw(1);
    let proj = vec![ProjectionElem::Field(field_idx)];
    let place = Place {
        local,
        projection: proj.into_boxed_slice(),
    };
    let rvalue = Rvalue::Use(Operand::Constant(MirConst {
        kind: MirConstKind::Int(42),
        ty: Ty::UNIT,
        span: Span::DUMMY,
    }));
    let stmt = Statement {
        kind: StatementKind::Assign(place, rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block = BasicBlockData {
        statements: vec![stmt],
        terminator: term,
        is_cleanup: false,
    };
    let mut bbs = IndexVec::new();
    bbs.push(block);
    let body = Arc::new(Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks: bbs,
        locals: IndexVec::new(),
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: Vec::new(),
    });
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    assert!(check_opcode(&bytecode, crate::OP_STORE_FIELD));
}

// U04-T02: Ref creates pointer
#[test]
fn test_ref_creates_pointer() {
    let backend = BytecodeBackend::new();
    let local = LocalIdx::from_raw(0);
    let place = Place::new(local);
    let rvalue = Rvalue::Ref(place, BorrowKind::Shared);
    let stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(1)), rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block = BasicBlockData {
        statements: vec![stmt],
        terminator: term,
        is_cleanup: false,
    };
    let mut bbs = IndexVec::new();
    bbs.push(block);
    let body = Arc::new(Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks: bbs,
        locals: IndexVec::new(),
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: Vec::new(),
    });
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    assert!(check_opcode(&bytecode, crate::OP_LOAD_LOCAL_ADDR));
}

// U04-T03: Deref loads from pointer
#[test]
fn test_deref_loads() {
    let backend = BytecodeBackend::new();
    let operand = Operand::Copy(Place::new(LocalIdx::from_raw(0)));
    let rvalue = Rvalue::UnaryOp(UnOp::Deref, operand);
    let stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(1)), rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block = BasicBlockData {
        statements: vec![stmt],
        terminator: term,
        is_cleanup: false,
    };
    let mut bbs = IndexVec::new();
    bbs.push(block);
    let body = Arc::new(Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks: bbs,
        locals: IndexVec::new(),
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: Vec::new(),
    });
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    assert!(check_opcode(&bytecode, crate::OP_DEREF));
}

// U04-T04: Drop calls glyim_drop_in_place
#[test]
fn test_drop_calls_drop_in_place() {
    let backend = BytecodeBackend::new();
    let target_bb = BasicBlockIdx::from_raw(1);
    let place = Place::new(LocalIdx::from_raw(0));
    let term = TerminatorKind::Drop {
        place: place.clone(),
        target: target_bb,
        cleanup: None,
    };
    let terminator = Terminator {
        kind: term,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block = BasicBlockData {
        statements: vec![],
        terminator,
        is_cleanup: false,
    };
    let mut bbs = IndexVec::new();
    bbs.push(block);
    let body = Arc::new(Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks: bbs,
        locals: IndexVec::new(),
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: Vec::new(),
    });
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    assert!(check_opcode(&bytecode, crate::OP_DROP));
    assert!(check_opcode(&bytecode, crate::OP_LOAD_LOCAL_ADDR));
    assert!(check_opcode(&bytecode, crate::OP_JUMP));
}

// U04-T05: Repeat builds array constant
#[test]
fn test_repeat_array_constant() {
    let backend = BytecodeBackend::new();
    let operand = Operand::Constant(MirConst {
        kind: MirConstKind::Int(42),
        ty: Ty::UNIT,
        span: Span::DUMMY,
    });
    let len_const = MirConst {
        kind: MirConstKind::Int(5),
        ty: Ty::UNIT,
        span: Span::DUMMY,
    };
    let rvalue = Rvalue::Repeat(operand, len_const);
    let stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(0)), rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block = BasicBlockData {
        statements: vec![stmt],
        terminator: term,
        is_cleanup: false,
    };
    let mut bbs = IndexVec::new();
    bbs.push(block);
    let body = Arc::new(Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks: bbs,
        locals: IndexVec::new(),
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: Vec::new(),
    });
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    assert!(check_opcode(&bytecode, crate::OP_REPEAT));
}

// U04-T06: Float constant emits correctly
#[test]
fn test_float_constant_emits() {
    let backend = BytecodeBackend::new();
    let float_const = MirConst {
        kind: MirConstKind::FloatBits(3.14159_f64.to_bits()),
        ty: Ty::UNIT,
        span: Span::DUMMY,
    };
    let rvalue = Rvalue::Use(Operand::Constant(float_const));
    let stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(0)), rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block = BasicBlockData {
        statements: vec![stmt],
        terminator: term,
        is_cleanup: false,
    };
    let mut bbs = IndexVec::new();
    bbs.push(block);
    let body = Arc::new(Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks: bbs,
        locals: IndexVec::new(),
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: Vec::new(),
    });
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    // Find OP_LOAD_CONST (0x01) and verify the next 8 bytes are the float bits
    let bits = 3.14159_f64.to_bits();
    let expected_bytes = bits.to_le_bytes();
    let mut found = false;
    for i in 0..bytecode.len().saturating_sub(8) {
        if bytecode[i] == crate::OP_LOAD_CONST {
            if &bytecode[i + 1..i + 9] == expected_bytes {
                found = true;
                break;
            }
        }
    }
    assert!(found, "Float constant not found in bytecode");
}
