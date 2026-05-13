//! Abstract code generation backend.

use glyim_core::primitives::BinOp;
use glyim_diag::CompResult;
use glyim_mir::*;
use glyim_type::Ty;
use std::path::Path;
use std::sync::Arc;

pub trait CodegenBackend {
    fn name(&self) -> &'static str;
    fn generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<Vec<u8>>;
    fn generate_function(&self, body: &Arc<Body>) -> CompResult<Vec<u8>>;
}

pub struct BytecodeBackend;

impl Default for BytecodeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl BytecodeBackend {
    pub fn new() -> Self {
        Self
    }
}

// Bytecode opcodes
const OP_LOAD_CONST: u8 = 0x01;
const OP_ADD: u8 = 0x02;
const OP_LOAD_LOCAL: u8 = 0x03;
const OP_STORE_LOCAL: u8 = 0x04;
const OP_RETURN: u8 = 0x05;
const OP_JUMP_IF: u8 = 0x06;
const OP_JUMP: u8 = 0x07;

impl CodegenBackend for BytecodeBackend {
    fn name(&self) -> &'static str {
        "bytecode"
    }

    fn generate(&self, bodies: &[Arc<Body>], _output: &Path) -> CompResult<Vec<u8>> {
        let mut combined = Vec::new();
        for body in bodies {
            let func_bc = self.generate_function(body)?;
            combined.extend(func_bc);
        }
        Ok(combined)
    }

    fn generate_function(&self, body: &Arc<Body>) -> CompResult<Vec<u8>> {
        let mut bc = Vec::new();
        for (bb_idx, block) in body.basic_blocks.iter_enumerated() {
            for stmt in &block.statements {
                emit_statement(&mut bc, &stmt.kind)?;
            }
            let term = &block.terminator;
            emit_terminator(&mut bc, &term.kind, bb_idx.to_raw())?;
        }
        Ok(bc)
    }
}

fn emit_statement(bc: &mut Vec<u8>, kind: &StatementKind) -> CompResult<()> {
    match kind {
        StatementKind::Assign(place, rvalue) => {
            emit_rvalue(bc, rvalue)?;
            if place.projection.is_empty() {
                bc.push(OP_STORE_LOCAL);
                bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
            } else {
                tracing::warn!("STUB: assign with non-empty projection not supported");
            }
        }
        StatementKind::StorageLive(_) => {
            tracing::debug!("StorageLive ignored in bytecode backend");
        }
        StatementKind::StorageDead(_) => {
            tracing::debug!("StorageDead ignored in bytecode backend");
        }
        StatementKind::Nop => {
            // nothing
        }
    }
    Ok(())
}

fn emit_rvalue(bc: &mut Vec<u8>, rvalue: &Rvalue) -> CompResult<()> {
    match rvalue {
        Rvalue::Use(operand) => {
            emit_operand(bc, operand);
        }
        Rvalue::BinaryOp(op, operands_box) => {
            let (left, right) = operands_box.as_ref();
            emit_operand(bc, left);
            emit_operand(bc, right);
            match op {
                BinOp::Add => bc.push(OP_ADD),
                BinOp::Sub => tracing::warn!("STUB: Sub not implemented"),
                BinOp::Mul => tracing::warn!("STUB: Mul not implemented"),
                BinOp::Div => tracing::warn!("STUB: Div not implemented"),
                _ => tracing::warn!("STUB: binary operator {:?} not implemented", op),
            }
        }
        Rvalue::Ref(_, _) => {
            tracing::warn!("STUB: Ref not implemented");
        }
        Rvalue::UnaryOp(_, _) => {
            tracing::warn!("STUB: UnaryOp not implemented");
        }
        _ => {
            tracing::warn!("STUB: rvalue {:?} not implemented", rvalue);
        }
    }
    Ok(())
}

fn emit_operand(bc: &mut Vec<u8>, operand: &Operand) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            if place.projection.is_empty() {
                bc.push(OP_LOAD_LOCAL);
                bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
            } else {
                tracing::warn!("STUB: operand with non-empty projection not supported");
            }
        }
        Operand::Constant(mir_const) => {
            bc.push(OP_LOAD_CONST);
            match &mir_const.kind {
                MirConstKind::Int(val) => {
                    bc.extend_from_slice(&(*val as i64).to_le_bytes());
                }
                MirConstKind::Bool(b) => {
                    bc.extend_from_slice(&(if *b { 1i64 } else { 0i64 }).to_le_bytes());
                }
                MirConstKind::Char(c) => {
                    bc.extend_from_slice(&(*c as i64).to_le_bytes());
                }
                MirConstKind::Uint(val) => {
                    bc.extend_from_slice(&(*val as i64).to_le_bytes());
                }
                MirConstKind::FloatBits(bits) => {
                    bc.extend_from_slice(&(*bits as i64).to_le_bytes());
                }
                _ => {
                    tracing::warn!("STUB: unsupported constant kind");
                    bc.extend_from_slice(&0i64.to_le_bytes());
                }
            }
        }
    }
}

fn emit_terminator(bc: &mut Vec<u8>, kind: &TerminatorKind, _bb_idx: u32) -> CompResult<()> {
    match kind {
        TerminatorKind::Return => {
            bc.push(OP_RETURN);
        }
        TerminatorKind::SwitchInt {
            discr,
            switch_ty,
            targets,
        } => {
            if *switch_ty == Ty::BOOL {
                emit_operand(bc, discr);
                // For bool: SwitchTargets contains one entry for false (value 0)
                // and the otherwise target for true.
                let false_target = targets
                    .iter()
                    .next()
                    .map(|(_, idx)| idx)
                    .unwrap_or_else(|| targets.otherwise());
                let true_target = targets.otherwise();
                // Jump if true (non-zero)
                bc.push(OP_JUMP_IF);
                bc.extend_from_slice(&true_target.to_raw().to_le_bytes());
                // Jump to false target
                bc.push(OP_JUMP);
                bc.extend_from_slice(&false_target.to_raw().to_le_bytes());
            } else {
                tracing::warn!("STUB: SwitchInt for non-bool type not implemented");
            }
        }
        TerminatorKind::Goto { target } => {
            bc.push(OP_JUMP);
            bc.extend_from_slice(&target.to_raw().to_le_bytes());
        }
        TerminatorKind::Call {
            func: _,
            args: _,
            destination: _,
            target: _,
            cleanup: _,
        } => {
            tracing::warn!("STUB: Call not implemented");
        }
        TerminatorKind::Unreachable => {
            // nothing
        }
        _ => {
            tracing::warn!("STUB: terminator {:?} not implemented", kind);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
