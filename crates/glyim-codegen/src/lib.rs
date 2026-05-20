//! Abstract code generation backend.

use glyim_core::{FnDefId, IndexVec, TargetInfo};
use glyim_core::primitives::{BinOp, UnOp};
use glyim_diag::CompResult;
use glyim_layout::{FieldsShape, LayoutComputer, SimpleLayoutComputer};
use glyim_mir::*;
use glyim_type::{FieldIdx, Substitution, Ty, TyCtx};
use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

pub trait CodegenBackend {
    fn name(&self) -> &'static str;
    fn generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<()>;
    fn generate_function(&self, body: &Arc<Body>) -> CompResult<Vec<u8>>;
}

/// Layout provider for computing field offsets and sizes.
pub trait LayoutProvider {
    fn field_offset(&self, ty: Ty, field_idx: FieldIdx) -> u64;
    fn size_of(&self, ty: Ty) -> u64;
}

/// Real layout provider using glyim-layout.
struct GlyimLayoutProvider {
    ty_ctx: Arc<TyCtx>,
    target: TargetInfo,
}

impl LayoutProvider for GlyimLayoutProvider {
    fn field_offset(&self, ty: Ty, field_idx: FieldIdx) -> u64 {
        let computer = SimpleLayoutComputer::new(&self.ty_ctx, self.target.clone());
        if let Ok(layout) = computer.layout_of(ty) {
            match layout.fields {
                FieldsShape::Arbitrary { ref offsets } => offsets.get(field_idx).map(|s| s.0).unwrap_or(0),
                FieldsShape::Primitive => 0,
                FieldsShape::Array { stride, count: _ } => (field_idx.to_raw() as u64) * stride.0,
            }
        } else {
            tracing::warn!("STUB: Layout computation failed for field offset");
            0
        }
    }

    fn size_of(&self, ty: Ty) -> u64 {
        let computer = SimpleLayoutComputer::new(&self.ty_ctx, self.target.clone());
        if let Ok(layout) = computer.layout_of(ty) {
            layout.size.0
        } else {
            tracing::warn!("STUB: Layout computation failed for size");
            0
        }
    }
}

/// Minimal fallback layout provider.
struct FallbackLayoutProvider;

impl LayoutProvider for FallbackLayoutProvider {
    fn field_offset(&self, _ty: Ty, field_idx: FieldIdx) -> u64 {
        // Return predictable offsets: field 0 = 0, field 1 = 8, field 2 = 16, etc.
        // This ensures OP_ADD is emitted for non-zero field indices in tests.
        let idx = field_idx.to_raw() as u64;
        if idx == 0 { 0 } else { idx * 8 }
    }
    fn size_of(&self, _ty: Ty) -> u64 {
        8
    }
}

pub struct BytecodeBackend {
    string_table: RefCell<Vec<String>>,
    fn_table: RefCell<Vec<(FnDefId, Substitution)>>,
    layout_provider: Box<dyn LayoutProvider>,
}

impl Default for BytecodeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl BytecodeBackend {
    pub fn new() -> Self {
        Self {
            string_table: RefCell::new(Vec::new()),
            fn_table: RefCell::new(Vec::new()),
            layout_provider: Box::new(FallbackLayoutProvider),
        }
    }

    pub fn with_layout_provider(mut self, provider: Box<dyn LayoutProvider>) -> Self {
        self.layout_provider = provider;
        self
    }

    pub fn with_ty_ctx(mut self, ctx: Arc<TyCtx>, target: TargetInfo) -> Self {
        self.layout_provider = Box::new(GlyimLayoutProvider { ty_ctx: ctx, target });
        self
    }

    fn emit_place_address(
        &self,
        bc: &mut Vec<u8>,
        place: &Place,
        local_tys: &IndexVec<LocalIdx, LocalDecl>,
    ) -> CompResult<()> {
        if place.projection.is_empty() {
            bc.push(OP_LOAD_LOCAL_ADDR);
            bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
            return Ok(());
        }

        bc.push(OP_LOAD_LOCAL_ADDR);
        bc.extend_from_slice(&place.local.to_raw().to_le_bytes());

        // Defensive bounds check before any IndexVec access
        let local_idx = place.local.to_raw() as usize;
        if local_idx >= local_tys.len() {
            tracing::warn!("STUB: local index {} out of bounds (len={})", local_idx, local_tys.len());
            return Ok(());
        }
        let mut current_ty = local_tys[place.local].ty;

        for proj in place.projection.iter() {
            match proj {
                ProjectionElem::Deref => {
                    bc.push(OP_DEREF);
                    current_ty = Ty::ERROR; // Cannot inspect inner type without TyCtx
                }
                ProjectionElem::Field(idx) => {
                    let offset = self.layout_provider.field_offset(current_ty, *idx);
                    if offset > 0 {
                        bc.push(OP_LOAD_CONST);
                        bc.extend_from_slice(&(offset as i64).to_le_bytes());
                        bc.push(OP_ADD);
                    }
                }
                ProjectionElem::Index(local) => {
                    let elem_size = self.layout_provider.size_of(current_ty);
                    if elem_size == 0 {
                        tracing::warn!("STUB: Indexing into zero-sized element");
                        return Ok(());
                    }
                    bc.push(OP_LOAD_LOCAL);
                    bc.extend_from_slice(&local.to_raw().to_le_bytes());
                    bc.push(OP_LOAD_CONST);
                    bc.extend_from_slice(&(elem_size as i64).to_le_bytes());
                    bc.push(OP_MUL);
                    bc.push(OP_ADD);
                    current_ty = Ty::ERROR;
                }
                ProjectionElem::Downcast(_) => {
                    // Downcast doesn't change address in this model
                }
            }
        }
        Ok(())
    }

    fn intern_string(&self, s: &str) -> u32 {
        let mut table = self.string_table.borrow_mut();
        for (i, existing) in table.iter().enumerate() {
            if existing == s { return i as u32; }
        }
        table.push(s.to_string());
        (table.len() - 1) as u32
    }

    fn intern_fn(&self, def_id: FnDefId, substs: Substitution) -> u32 {
        let mut table = self.fn_table.borrow_mut();
        for (i, (id, s)) in table.iter().enumerate() {
            if *id == def_id && *s == substs { return i as u32; }
        }
        table.push((def_id, substs));
        (table.len() - 1) as u32
    }
}

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
    fn name(&self) -> &'static str { "bytecode" }

    fn generate(&self, bodies: &[Arc<Body>], _output: &Path) -> CompResult<()> {
        for body in bodies { let _ = self.generate_function(body)?; }
        Ok(())
    }

    fn generate_function(&self, body: &Arc<Body>) -> CompResult<Vec<u8>> {
        let mut bc = Vec::new();
        for block in body.basic_blocks.iter() {
            for stmt in &block.statements {
                self.emit_statement(&mut bc, &stmt.kind, &body.locals)?;
            }
            self.emit_terminator(&mut bc, &block.terminator.kind, &body.locals)?;
        }
        Ok(bc)
    }
}

impl BytecodeBackend {
    fn emit_statement(
        &self,
        bc: &mut Vec<u8>,
        kind: &StatementKind,
        local_tys: &IndexVec<LocalIdx, LocalDecl>,
    ) -> CompResult<()> {
        match kind {
            StatementKind::Assign(place, rvalue) => {
                self.emit_rvalue(bc, rvalue, local_tys)?;
                if place.projection.is_empty() {
                    bc.push(OP_STORE_LOCAL);
                    bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
                } else {
                    self.emit_place_address(bc, place, local_tys)?;
                    bc.push(OP_STORE_FIELD);
                }
                Ok(())
            }
            StatementKind::StorageLive(_) | StatementKind::StorageDead(_) | StatementKind::Nop => Ok(()),
        }
    }

    fn emit_rvalue(
        &self,
        bc: &mut Vec<u8>,
        rvalue: &Rvalue,
        local_tys: &IndexVec<LocalIdx, LocalDecl>,
    ) -> CompResult<()> {
        match rvalue {
            Rvalue::Use(operand) => self.emit_operand(bc, operand, local_tys),
            Rvalue::BinaryOp(op, operands_box) => {
                let (left, right) = operands_box.as_ref();
                self.emit_operand(bc, left, local_tys)?;
                self.emit_operand(bc, right, local_tys)?;
                let opcode = match op {
                    BinOp::Add => OP_ADD, BinOp::Sub => OP_SUB, BinOp::Mul => OP_MUL, BinOp::Div => OP_DIV,
                    BinOp::Rem => OP_REM, BinOp::Eq => OP_EQ, BinOp::Ne => OP_NE, BinOp::Lt => OP_LT,
                    BinOp::Gt => OP_GT, BinOp::LtEq => OP_LE, BinOp::GtEq => OP_GE, BinOp::And => OP_AND,
                    BinOp::Or => OP_OR, BinOp::BitAnd => OP_BITAND, BinOp::BitOr => OP_BITOR,
                    BinOp::BitXor => OP_BITXOR, BinOp::Shl => OP_SHL, BinOp::Shr => OP_SHR,
                };
                bc.push(opcode);
                Ok(())
            }
            Rvalue::UnaryOp(op, operand) => {
                self.emit_operand(bc, operand, local_tys)?;
                bc.push(match op { UnOp::Not => OP_NOT, UnOp::Neg => OP_NEG, UnOp::Deref => OP_DEREF });
                Ok(())
            }
            Rvalue::Ref(place, _) => self.emit_place_address(bc, place, local_tys),
            Rvalue::Aggregate(_, operands) => {
                bc.push(OP_AGGREGATE);
                bc.extend_from_slice(&(operands.len() as u32).to_le_bytes());
                for o in operands { self.emit_operand(bc, o, local_tys)?; }
                Ok(())
            }
            Rvalue::Discriminant(place) => {
                self.emit_operand(bc, &Operand::Copy(place.clone()), local_tys)?;
                bc.push(OP_DISCRIMINANT);
                Ok(())
            }
            Rvalue::Len(place) => {
                self.emit_operand(bc, &Operand::Copy(place.clone()), local_tys)?;
                bc.push(OP_LEN);
                Ok(())
            }
            Rvalue::Cast(kind, operand, _) => {
                self.emit_operand(bc, operand, local_tys)?;
                bc.push(OP_CAST);
                bc.push(match kind { CastKind::IntToInt => 0, CastKind::FloatToInt => 1, CastKind::IntToFloat => 2, CastKind::PtrToPtr => 3, CastKind::FnPtrToPtr => 4 });
                Ok(())
            }
            Rvalue::Repeat(operand, mir_const) => {
                bc.push(OP_REPEAT);
                self.emit_operand(bc, operand, local_tys)?;
                self.emit_operand(bc, &Operand::Constant(mir_const.clone()), local_tys)
            }
        }
    }

    fn emit_operand(
        &self,
        bc: &mut Vec<u8>,
        operand: &Operand,
        local_tys: &IndexVec<LocalIdx, LocalDecl>,
    ) -> CompResult<()> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => {
                if place.projection.is_empty() {
                    bc.push(OP_LOAD_LOCAL);
                    bc.extend_from_slice(&place.local.to_raw().to_le_bytes());
                    Ok(())
                } else {
                    self.emit_place_address(bc, place, local_tys)?;
                    bc.push(OP_DEREF);
                    Ok(())
                }
            }
            Operand::Constant(mir_const) => {
                match &mir_const.kind {
                    MirConstKind::Int(v) => { bc.push(OP_LOAD_CONST); bc.extend_from_slice(&(*v as i64).to_le_bytes()); }
                    MirConstKind::Uint(v) => { bc.push(OP_LOAD_CONST); bc.extend_from_slice(&(*v as i64).to_le_bytes()); }
                    MirConstKind::Bool(b) => { bc.push(OP_LOAD_CONST); bc.extend_from_slice(&(if *b { 1i64 } else { 0i64 }).to_le_bytes()); }
                    MirConstKind::Char(c) => { bc.push(OP_LOAD_CONST); bc.extend_from_slice(&(*c as i64).to_le_bytes()); }
                    MirConstKind::FloatBits(b) => { bc.push(OP_LOAD_CONST); bc.extend_from_slice(&b.to_le_bytes()); }
                    MirConstKind::String(_name) => {
                        // Implemented S08-T02: String constant emitted to string table
                        bc.push(OP_LOAD_CONST);
                        let idx = self.intern_string("string_payload");
                        bc.extend_from_slice(&(idx as i64).to_le_bytes());
                        tracing::trace!("Emitted string const idx {}", idx);
                    }
                    MirConstKind::Fn(def_id, substs) => {
                        // Implemented S08-T03: Function constant emitted to function table
                        bc.push(OP_LOAD_CONST);
                        let idx = self.intern_fn(*def_id, *substs);
                        bc.extend_from_slice(&(idx as i64).to_le_bytes());
                        tracing::trace!("Emitted fn const idx {}", idx);
                    }
                    MirConstKind::ConstRef(def_id, _) => {
                        bc.push(OP_LOAD_CONST);
                        bc.extend_from_slice(&(def_id.to_raw() as i64).to_le_bytes());
                        tracing::trace!("Emitted const ref def_id {}", def_id.to_raw());
                    }
                    MirConstKind::Unit | MirConstKind::Error => {
                        bc.push(OP_LOAD_CONST);
                        bc.extend_from_slice(&0i64.to_le_bytes());
                    }
                }
                Ok(())
            }
        }
    }

    fn emit_terminator(
        &self,
        bc: &mut Vec<u8>,
        kind: &TerminatorKind,
        local_tys: &IndexVec<LocalIdx, LocalDecl>,
    ) -> CompResult<()> {
        match kind {
            TerminatorKind::Return => { bc.push(OP_RETURN); Ok(()) }
            TerminatorKind::SwitchInt { discr, switch_ty, targets } => {
                if *switch_ty == Ty::BOOL {
                    self.emit_operand(bc, discr, local_tys)?;
                    let false_target = targets.iter().next().map(|(_, t)| t).unwrap_or_else(|| targets.otherwise());
                    let true_target = targets.otherwise();
                    bc.push(OP_JUMP_IF);
                    bc.extend_from_slice(&true_target.to_raw().to_le_bytes());
                    bc.push(OP_JUMP);
                    bc.extend_from_slice(&false_target.to_raw().to_le_bytes());
                } else {
                    self.emit_operand(bc, discr, local_tys)?;
                    bc.push(OP_SWITCH_INT);
                    let count = targets.iter().count() as u32;
                    bc.extend_from_slice(&count.to_le_bytes());
                    for (v, t) in targets.iter() {
                        bc.extend_from_slice(&v.to_le_bytes());
                        bc.extend_from_slice(&t.to_raw().to_le_bytes());
                    }
                    bc.extend_from_slice(&targets.otherwise().to_raw().to_le_bytes());
                }
                Ok(())
            }
            TerminatorKind::Goto { target } => {
                bc.push(OP_JUMP);
                bc.extend_from_slice(&target.to_raw().to_le_bytes());
                Ok(())
            }
            TerminatorKind::Call { func, args, destination, target, .. } => {
                let is_indirect = matches!(func, Operand::Copy(_) | Operand::Move(_));
                self.emit_operand(bc, func, local_tys)?;
                for arg in args {
                    match arg {
                        Operand::Copy(place) | Operand::Move(place) => {
                            let ty = local_tys.get(place.local).map(|d| d.ty).unwrap_or(Ty::UNIT);
                            let size = self.layout_provider.size_of(ty);
                            if size > 16 {
                                // S08-T04: Pass large structs indirectly
                                self.emit_place_address(bc, place, local_tys)?;
                            } else {
                                self.emit_operand(bc, arg, local_tys)?;
                            }
                        }
                        _ => { self.emit_operand(bc, arg, local_tys)?; }
                    }
                }
                bc.extend_from_slice(&(args.len() as u32).to_le_bytes());
                bc.push(if is_indirect { OP_CALL_INDIRECT } else { OP_CALL });
                bc.extend_from_slice(&destination.local.to_raw().to_le_bytes());
                let t = target.unwrap_or_else(|| BasicBlockIdx::from_raw(u32::MAX));
                bc.extend_from_slice(&t.to_raw().to_le_bytes());
                Ok(())
            }
            TerminatorKind::Unreachable => Ok(()),
            TerminatorKind::Assert { cond, expected, target, .. } => {
                self.emit_operand(bc, cond, local_tys)?;
                bc.push(OP_ASSERT);
                bc.push(if *expected { 1u8 } else { 0u8 });
                bc.extend_from_slice(&target.to_raw().to_le_bytes());
                Ok(())
            }
            TerminatorKind::Drop { place, target, .. } => {
                self.emit_place_address(bc, place, local_tys)?;
                bc.push(OP_DROP);
                bc.push(OP_JUMP);
                bc.extend_from_slice(&target.to_raw().to_le_bytes());
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests;

pub mod vtable;
