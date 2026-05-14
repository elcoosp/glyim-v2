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
                return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(
                    "bytecode backend: assign with non-empty projection not yet implemented",
                )]);
            }
        }
        StatementKind::StorageLive(_) => {
            // Valid no-op for bytecode
        }
        StatementKind::StorageDead(_) => {
            // Valid no-op for bytecode
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
                BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Rem
                | BinOp::Eq
                | BinOp::Ne
                | BinOp::Lt
                | BinOp::Gt
                | BinOp::LtEq
                | BinOp::GtEq
                | BinOp::And
                | BinOp::Or
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor
                | BinOp::Shl
                | BinOp::Shr => {
                    return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(format!(
                        "bytecode backend: binary operator {:?} not yet implemented",
                        op
                    ))]);
                }
            }
        }
        Rvalue::Ref(_, _) => {
            return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(
                "bytecode backend: Rvalue::Ref not yet implemented",
            )]);
        }
        Rvalue::UnaryOp(_, _) => {
            return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(
                "bytecode backend: Rvalue::UnaryOp not yet implemented",
            )]);
        }
        Rvalue::Aggregate(_, _) => {
            return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(
                "bytecode backend: Rvalue::Aggregate not yet implemented",
            )]);
        }
        Rvalue::Discriminant(_) => {
            return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(
                "bytecode backend: Rvalue::Discriminant not yet implemented",
            )]);
        }
        Rvalue::Len(_) => {
            return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(
                "bytecode backend: Rvalue::Len not yet implemented",
            )]);
        }
        Rvalue::Cast(_, _, _) => {
            return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(
                "bytecode backend: Rvalue::Cast not yet implemented",
            )]);
        }
        Rvalue::Repeat(_, _) => {
            return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(
                "bytecode backend: Rvalue::Repeat not yet implemented",
            )]);
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
            } else {
                return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(
                    "bytecode backend: SwitchInt for non-bool type not yet implemented",
                )]);
            }
        }
        TerminatorKind::Goto { target } => {
            bc.push(OP_JUMP);
            bc.extend_from_slice(&target.to_raw().to_le_bytes());
        }
        TerminatorKind::Call { .. } => {
            return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(
                "bytecode backend: Call terminator not yet implemented",
            )]);
        }
        TerminatorKind::Unreachable => {
            // Valid no-op: unreachable code produces no bytecode
        }
        TerminatorKind::Assert { .. } | TerminatorKind::Drop { .. } => {
            return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(format!(
                "bytecode backend: terminator {:?} not yet implemented",
                kind
            ))]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyim_core::def_id::{CrateId, DefId, LocalDefId};
    use std::sync::Arc;

    #[test]
    fn test_unsupported_ref_returns_error() {
        let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
        let local_idx = LocalIdx::from_raw(1);
        body.locals.push(glyim_mir::LocalDecl {
            ty: glyim_type::Ty::ERROR,
            mutability: glyim_core::primitives::Mutability::Not,
            source_info: glyim_mir::SourceInfo::new(glyim_span::Span::DUMMY),
        });
        body.basic_blocks[BasicBlockIdx::from_raw(0)]
            .statements
            .push(glyim_mir::Statement {
                kind: glyim_mir::StatementKind::Assign(
                    glyim_mir::Place::new(local_idx),
                    glyim_mir::Rvalue::Ref(
                        glyim_mir::Place::new(LocalIdx::from_raw(0)),
                        glyim_mir::BorrowKind::Shared,
                    ),
                ),
                source_info: glyim_mir::SourceInfo::new(glyim_span::Span::DUMMY),
            });

        let backend = BytecodeBackend::new();
        let result = backend.generate_function(&Arc::new(body));
        assert!(
            result.is_err(),
            "Rvalue::Ref should return Err, not silently produce wrong bytecode"
        );
    }

    #[test]
    fn test_unsupported_call_returns_error() {
        let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
        body.basic_blocks[BasicBlockIdx::from_raw(0)].terminator = Terminator {
            kind: TerminatorKind::Call {
                func: Operand::Constant(MirConst {
                    kind: MirConstKind::Int(0),
                    ty: glyim_type::Ty::ERROR,
                    span: glyim_span::Span::DUMMY,
                }),
                args: Vec::new(),
                destination: Place::new(LocalIdx::from_raw(0)),
                target: Some(BasicBlockIdx::from_raw(1)),
                cleanup: None,
            },
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        };

        let backend = BytecodeBackend::new();
        let result = backend.generate_function(&Arc::new(body));
        assert!(result.is_err(), "Call terminator should return Err");
    }

    #[test]
    fn test_unsupported_projection_returns_error() {
        let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
        let local_idx = LocalIdx::from_raw(1);
        body.locals.push(glyim_mir::LocalDecl {
            ty: glyim_type::Ty::ERROR,
            mutability: glyim_core::primitives::Mutability::Not,
            source_info: glyim_mir::SourceInfo::new(glyim_span::Span::DUMMY),
        });
        let place_with_proj = Place {
            local: local_idx,
            projection: Box::new([ProjectionElem::Deref]),
        };
        body.basic_blocks[BasicBlockIdx::from_raw(0)]
            .statements
            .push(glyim_mir::Statement {
                kind: glyim_mir::StatementKind::Assign(
                    place_with_proj,
                    glyim_mir::Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(1),
                        ty: glyim_type::Ty::ERROR,
                        span: glyim_span::Span::DUMMY,
                    })),
                ),
                source_info: glyim_mir::SourceInfo::new(glyim_span::Span::DUMMY),
            });

        let backend = BytecodeBackend::new();
        let result = backend.generate_function(&Arc::new(body));
        assert!(result.is_err(), "Non-empty projection should return Err");
    }

    #[test]
    fn test_dummy_body_succeeds() {
        let body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
        let backend = BytecodeBackend::new();
        let result = backend.generate_function(&Arc::new(body));
        assert!(result.is_ok(), "Dummy body should compile successfully");
    }
}
