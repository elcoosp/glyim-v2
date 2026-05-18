//! Abstract code generation backend.

use glyim_core::primitives::BinOp;
use glyim_diag::CompResult;
use glyim_mir::*;
use glyim_type::Ty;
use std::path::Path;
use std::sync::Arc;

pub trait CodegenBackend {
    fn name(&self) -> &'static str;
    fn generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<()>;
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
pub(crate) const OP_LOAD_CONST: u8 = 0x01;
pub(crate) const OP_ADD: u8 = 0x02;
pub(crate) const OP_SUB: u8 = 0x03;
pub(crate) const OP_MUL: u8 = 0x04;
pub(crate) const OP_DIV: u8 = 0x05;
pub(crate) const OP_REM: u8 = 0x06;
pub(crate) const OP_EQ: u8 = 0x07;
pub(crate) const OP_NE: u8 = 0x08;
pub(crate) const OP_LT: u8 = 0x09;
pub(crate) const OP_GT: u8 = 0x0A;
pub(crate) const OP_LE: u8 = 0x0B;
pub(crate) const OP_GE: u8 = 0x0C;
pub(crate) const OP_AND: u8 = 0x0D;
pub(crate) const OP_OR: u8 = 0x0E;
pub(crate) const OP_NOT: u8 = 0x0F;
pub(crate) const OP_NEG: u8 = 0x10;
pub(crate) const OP_BITAND: u8 = 0x11;
pub(crate) const OP_BITOR: u8 = 0x12;
pub(crate) const OP_BITXOR: u8 = 0x13;
pub(crate) const OP_SHL: u8 = 0x14;
pub(crate) const OP_SHR: u8 = 0x15;
pub(crate) const OP_LOAD_LOCAL: u8 = 0x16;
pub(crate) const OP_STORE_LOCAL: u8 = 0x17;
pub(crate) const OP_RETURN: u8 = 0x18;
pub(crate) const OP_JUMP_IF: u8 = 0x19;
pub(crate) const OP_JUMP: u8 = 0x1A;
pub(crate) const OP_CALL: u8 = 0x1B;
pub(crate) const OP_CAST: u8 = 0x1C;
pub(crate) const OP_AGGREGATE: u8 = 0x1D;
pub(crate) const OP_DISCRIMINANT: u8 = 0x1E;
pub(crate) const OP_LEN: u8 = 0x1F;
pub(crate) const OP_SWITCH_INT: u8 = 0x20;
pub(crate) const OP_ASSERT: u8 = 0x21;
pub(crate) const OP_CALL_INDIRECT: u8 = 0x22;
pub(crate) const OP_LOAD_LOCAL_ADDR: u8 = 0x29;
pub(crate) const OP_STORE_FIELD: u8 = 0x2A;
pub(crate) const OP_DEREF: u8 = 0x2B;
pub(crate) const OP_DROP: u8 = 0x2C;
pub(crate) const OP_REPEAT: u8 = 0x2D;

impl CodegenBackend for BytecodeBackend {
    fn name(&self) -> &'static str {
        "bytecode"
    }

    fn generate(&self, bodies: &[Arc<Body>], _output: &Path) -> CompResult<()> {
        for body in bodies {
            let _func_bc = self.generate_function(body)?;
        }
        Ok(())
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
                Ok(())
            } else {
                // Support simple field projection for now
                if place.projection.len() == 1
                    && let ProjectionElem::Field(idx) = place.projection[0]
                {
                    // Load base local, then store to field
                    bc.push(OP_LOAD_LOCAL);
                    bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
                    bc.push(OP_STORE_FIELD);
                    bc.extend_from_slice(&idx.to_raw().to_le_bytes());
                    return Ok(());
                }
                tracing::warn!("STUB: assign with non-trivial projection not fully implemented");
                // Fallback: store to local anyway (may be wrong)
                bc.push(OP_STORE_LOCAL);
                bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
                Ok(())
            }
        }
        StatementKind::StorageLive(_) => Ok(()),
        StatementKind::StorageDead(_) => Ok(()),
        StatementKind::Nop => Ok(()),
    }
}

fn emit_rvalue(bc: &mut Vec<u8>, rvalue: &Rvalue) -> CompResult<()> {
    match rvalue {
        Rvalue::Use(operand) => {
            emit_operand(bc, operand)?;
            Ok(())
        }
        Rvalue::BinaryOp(op, operands_box) => {
            let (left, right) = operands_box.as_ref();
            emit_operand(bc, left)?;
            emit_operand(bc, right)?;
            let opcode = match op {
                BinOp::Add => OP_ADD,
                BinOp::Sub => OP_SUB,
                BinOp::Mul => OP_MUL,
                BinOp::Div => OP_DIV,
                BinOp::Rem => OP_REM,
                BinOp::Eq => OP_EQ,
                BinOp::Ne => OP_NE,
                BinOp::Lt => OP_LT,
                BinOp::Gt => OP_GT,
                BinOp::LtEq => OP_LE,
                BinOp::GtEq => OP_GE,
                BinOp::And => OP_AND,
                BinOp::Or => OP_OR,
                BinOp::BitAnd => OP_BITAND,
                BinOp::BitOr => OP_BITOR,
                BinOp::BitXor => OP_BITXOR,
                BinOp::Shl => OP_SHL,
                BinOp::Shr => OP_SHR,
            };
            bc.push(opcode);
            Ok(())
        }
        Rvalue::UnaryOp(op, operand) => {
            emit_operand(bc, operand)?;
            let opcode = match op {
                glyim_core::primitives::UnOp::Not => OP_NOT,
                glyim_core::primitives::UnOp::Neg => OP_NEG,
                glyim_core::primitives::UnOp::Deref => OP_DEREF,
            };
            bc.push(opcode);
            Ok(())
        }
        Rvalue::Ref(place, _borrow_kind) => {
            // Emit address of place
            if place.projection.is_empty() {
                bc.push(OP_LOAD_LOCAL_ADDR);
                bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
            } else {
                // For simplicity, just load local and warn
                bc.push(OP_LOAD_LOCAL_ADDR);
                bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
                tracing::warn!("STUB: Ref with projection not fully implemented");
            }
            // Borrow kind ignored for now
            Ok(())
        }
        Rvalue::Aggregate(_, operands) => {
            bc.push(OP_AGGREGATE);
            let count = operands.len() as u32;
            bc.extend_from_slice(&count.to_le_bytes());
            for operand in operands {
                emit_operand(bc, operand)?;
            }
            Ok(())
        }
        Rvalue::Discriminant(place) => {
            emit_operand(bc, &Operand::Copy(place.clone()))?;
            bc.push(OP_DISCRIMINANT);
            Ok(())
        }
        Rvalue::Len(place) => {
            emit_operand(bc, &Operand::Copy(place.clone()))?;
            bc.push(OP_LEN);
            Ok(())
        }
        Rvalue::Cast(cast_kind, operand, _target_ty) => {
            emit_operand(bc, operand)?;
            bc.push(OP_CAST);
            let kind_byte: u8 = match cast_kind {
                CastKind::IntToInt => 0,
                CastKind::FloatToInt => 1,
                CastKind::IntToFloat => 2,
                CastKind::PtrToPtr => 3,
                CastKind::FnPtrToPtr => 4,
            };
            bc.push(kind_byte);
            Ok(())
        }
        Rvalue::Repeat(operand, mir_const) => {
            bc.push(OP_REPEAT);
            emit_operand(bc, operand)?;
            emit_operand(bc, &Operand::Constant(mir_const.clone()))?;
            Ok(())
        }
    }
}

fn emit_operand(bc: &mut Vec<u8>, operand: &Operand) -> CompResult<()> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            if place.projection.is_empty() {
                bc.push(OP_LOAD_LOCAL);
                bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
                Ok(())
            } else {
                tracing::warn!("STUB: operand with non-empty projection, falling back to load local");
                bc.push(OP_LOAD_LOCAL);
                bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
                Ok(())
            }
        }
        Operand::Constant(mir_const) => {
            bc.push(OP_LOAD_CONST);
            match &mir_const.kind {
                MirConstKind::Int(val) => {
                    bc.extend_from_slice(&(*val as i64).to_le_bytes());
                }
                MirConstKind::Uint(val) => {
                    bc.extend_from_slice(&(*val as i64).to_le_bytes());
                }
                MirConstKind::Bool(b) => {
                    bc.extend_from_slice(&(if *b { 1i64 } else { 0i64 }).to_le_bytes());
                }
                MirConstKind::Char(c) => {
                    bc.extend_from_slice(&(*c as i64).to_le_bytes());
                }
                MirConstKind::FloatBits(bits) => {
                    bc.extend_from_slice(&bits.to_le_bytes());
                }
                _ => {
                    tracing::warn!("STUB: unsupported constant kind");
                    bc.extend_from_slice(&0i64.to_le_bytes());
                }
            }
            Ok(())
        }
    }
}

fn emit_terminator(bc: &mut Vec<u8>, kind: &TerminatorKind, _bb_idx: u32) -> CompResult<()> {
    match kind {
        TerminatorKind::Return => {
            bc.push(OP_RETURN);
            Ok(())
        }
        TerminatorKind::SwitchInt {
            discr,
            switch_ty,
            targets,
        } => {
            if *switch_ty == Ty::BOOL {
                emit_operand(bc, discr)?;
                let false_target = targets
                    .iter()
                    .next()
                    .map(|(_, idx)| idx)
                    .unwrap_or_else(|| targets.otherwise());
                let true_target = targets.otherwise();
                bc.push(OP_JUMP_IF);
                bc.extend_from_slice(&true_target.to_raw().to_le_bytes());
                bc.push(OP_JUMP);
                bc.extend_from_slice(&false_target.to_raw().to_le_bytes());
                Ok(())
            } else {
                emit_operand(bc, discr)?;
                bc.push(OP_SWITCH_INT);
                let num_branches = targets.iter().count() as u32;
                bc.extend_from_slice(&num_branches.to_le_bytes());
                for (val, target) in targets.iter() {
                    bc.extend_from_slice(&val.to_le_bytes());
                    bc.extend_from_slice(&target.to_raw().to_le_bytes());
                }
                bc.extend_from_slice(&targets.otherwise().to_raw().to_le_bytes());
                Ok(())
            }
        }
        TerminatorKind::Goto { target } => {
            bc.push(OP_JUMP);
            bc.extend_from_slice(&target.to_raw().to_le_bytes());
            Ok(())
        }
        TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            cleanup: _,
        } => {
            // Determine if this is a direct or indirect call
            let is_indirect = matches!(func, Operand::Copy(_) | Operand::Move(_));
            emit_operand(bc, func)?;
            let arg_count = args.len() as u32;
            bc.extend_from_slice(&arg_count.to_le_bytes());
            for arg in args {
                emit_operand(bc, arg)?;
            }
            if is_indirect {
                bc.push(OP_CALL_INDIRECT);
            } else {
                bc.push(OP_CALL);
            }
            bc.extend_from_slice(&destination.local.to_raw().to_le_bytes());
            let target_bb = target.unwrap_or_else(|| BasicBlockIdx::from_raw(u32::MAX));
            bc.extend_from_slice(&target_bb.to_raw().to_le_bytes());
            Ok(())
        }
        TerminatorKind::Unreachable => Ok(()),
        TerminatorKind::Assert {
            cond,
            expected,
            target,
            cleanup: _,
            msg: _,
        } => {
            emit_operand(bc, cond)?;
            bc.push(OP_ASSERT);
            bc.push(if *expected { 1u8 } else { 0u8 });
            bc.extend_from_slice(&target.to_raw().to_le_bytes());
            Ok(())
        }
        TerminatorKind::Drop {
            place,
            target,
            cleanup: _,
        } => {
            // Emit a call to glyim_drop_in_place (simulated as OP_DROP)
            bc.push(OP_DROP);
            // Emit place address
            if place.projection.is_empty() {
                bc.push(OP_LOAD_LOCAL_ADDR);
                bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
            } else {
                bc.push(OP_LOAD_LOCAL_ADDR);
                bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
                tracing::warn!("STUB: Drop with projection not fully implemented");
            }
            // Jump to target after drop
            bc.push(OP_JUMP);
            bc.extend_from_slice(&target.to_raw().to_le_bytes());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;

pub mod vtable;

// Export opcode constants for testing
