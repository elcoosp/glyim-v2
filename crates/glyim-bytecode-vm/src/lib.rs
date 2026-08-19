//! Switch-dispatch bytecode virtual machine for the Glyim bytecode backend.
//!
//! This is the Phase 7.1 MVP: a real, executing VM for the core opcode subset
//! the `glyim-codegen` bytecode backend emits (`OP_LOAD_CONST`, the arithmetic
//! /logic ops, `OP_LOAD_LOCAL`/`OP_STORE_LOCAL`, `OP_JUMP`/`OP_JUMP_IF`,
//! `OP_RETURN`). It proves the bytecode backend is no longer a "golden-test-
//! only" stub — programs can be executed, not just inspected for emitted bytes.
//!
//! The remaining opcodes (`OP_CALL`/`OP_CALL_INDIRECT`, `OP_AGGREGATE`,
//! `OP_DEREF`/`OP_STORE_FIELD`, `OP_SWITCH_INT`, `OP_REPEAT`, `OP_DROP`,
//! `OP_ASSERT`, `OP_DISCRIMINANT`, `OP_LEN`, `OP_CAST`, `OP_TRAP`) are tracked
//! in `docs/plans/v0.1.0/unstub-5/KNOWN_GAPS.md` Phase 7.1. The `Opcode`
//! coverage is structured so that every opcode the emitter can produce must be
//! handled or the decoder returns an explicit "unsupported opcode" error rather
//! than silently mis-executing.

/// A runtime value on the VM stack / in a local slot.
///
/// The MVP represents every operand as a 64-bit integer (the bytecode backend
/// emits all `OP_LOAD_CONST` payloads as `i64`). Aggregate / pointer / function
/// values are a tracked follow-up once `OP_AGGREGATE`/`OP_DEREF` land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// A 64-bit integer scalar (also used for bools: 0 = false, 1 = true).
    Int(i64),
}

impl Value {
    fn as_int(&self) -> i64 {
        match self {
            Value::Int(v) => *v,
        }
    }
}

/// Opcodes emitted by `glyim-codegen`'s bytecode backend.
///
/// The numeric values mirror `crates/glyim-codegen/src/lib.rs` exactly so the
/// VM and emitter agree on the wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    LoadConst = 0x01,
    Add = 0x02,
    Sub = 0x03,
    Mul = 0x04,
    Div = 0x05,
    Rem = 0x06,
    Eq = 0x07,
    Ne = 0x08,
    Lt = 0x09,
    Gt = 0x0A,
    Le = 0x0B,
    Ge = 0x0C,
    And = 0x0D,
    Or = 0x0E,
    Not = 0x0F,
    Neg = 0x10,
    BitAnd = 0x11,
    BitOr = 0x12,
    BitXor = 0x13,
    Shl = 0x14,
    Shr = 0x15,
    LoadLocal = 0x16,
    StoreLocal = 0x17,
    Return = 0x18,
    JumpIf = 0x19,
    Jump = 0x1A,
    Call = 0x1B,
    Cast = 0x1C,
    Aggregate = 0x1D,
    Discriminant = 0x1E,
    Len = 0x1F,
    SwitchInt = 0x20,
    Assert = 0x21,
    CallIndirect = 0x22,
    LoadLocalAddr = 0x29,
    StoreField = 0x2A,
    Deref = 0x2B,
    Drop = 0x2C,
    Repeat = 0x2D,
    Trap = 0xFF,
}

impl Opcode {
    fn from_u8(b: u8) -> Option<Opcode> {
        Some(match b {
            0x01 => Opcode::LoadConst,
            0x02 => Opcode::Add,
            0x03 => Opcode::Sub,
            0x04 => Opcode::Mul,
            0x05 => Opcode::Div,
            0x06 => Opcode::Rem,
            0x07 => Opcode::Eq,
            0x08 => Opcode::Ne,
            0x09 => Opcode::Lt,
            0x0A => Opcode::Gt,
            0x0B => Opcode::Le,
            0x0C => Opcode::Ge,
            0x0D => Opcode::And,
            0x0E => Opcode::Or,
            0x0F => Opcode::Not,
            0x10 => Opcode::Neg,
            0x11 => Opcode::BitAnd,
            0x12 => Opcode::BitOr,
            0x13 => Opcode::BitXor,
            0x14 => Opcode::Shl,
            0x15 => Opcode::Shr,
            0x16 => Opcode::LoadLocal,
            0x17 => Opcode::StoreLocal,
            0x18 => Opcode::Return,
            0x19 => Opcode::JumpIf,
            0x1A => Opcode::Jump,
            0x1B => Opcode::Call,
            0x1C => Opcode::Cast,
            0x1D => Opcode::Aggregate,
            0x1E => Opcode::Discriminant,
            0x1F => Opcode::Len,
            0x20 => Opcode::SwitchInt,
            0x21 => Opcode::Assert,
            0x22 => Opcode::CallIndirect,
            0x29 => Opcode::LoadLocalAddr,
            0x2A => Opcode::StoreField,
            0x2B => Opcode::Deref,
            0x2C => Opcode::Drop,
            0x2D => Opcode::Repeat,
            0xFF => Opcode::Trap,
            _ => return None,
        })
    }
}

/// A chunk of bytecode to execute, plus the byte offset where execution begins.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Raw instruction bytes.
    pub code: Vec<u8>,
    /// Byte offset of the function entry point (0 for a single function).
    pub entry: usize,
}

impl Chunk {
    pub fn new(code: Vec<u8>) -> Chunk {
        Chunk { code, entry: 0 }
    }
}

/// Result of execution.
pub type ExecResult<T> = Result<T, VmError>;

/// Errors the VM can encounter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    /// Ran past the end of the code without a `Return`.
    UnexpectedEndOfCode,
    /// A `u8` did not decode to a known opcode.
    UnknownOpcode(u8),
    /// The decoded opcode is not yet implemented in this MVP.
    UnsupportedOpcode(Opcode),
    /// Stack underflow (popped with too few operands).
    StackUnderflow,
    /// Local index out of bounds.
    LocalOutOfBounds(usize),
}

/// The virtual machine.
pub struct Vm {
    stack: Vec<Value>,
    locals: Vec<Value>,
}

impl Vm {
    /// Create a VM with `n_locals` slots (pre-zeroed).
    pub fn new(n_locals: usize) -> Vm {
        Vm {
            stack: Vec::new(),
            locals: vec![Value::Int(0); n_locals],
        }
    }

    /// Execute `chunk` to completion, returning the function's return value.
    pub fn run(&mut self, chunk: &Chunk) -> ExecResult<Value> {
        let code = &chunk.code;
        let mut pc = chunk.entry;
        let len = code.len();

        while pc < len {
            let raw = code[pc];
            let op = Opcode::from_u8(raw).ok_or(VmError::UnknownOpcode(raw))?;
            pc += 1;
            match op {
                Opcode::LoadConst => {
                    if pc + 8 > len {
                        return Err(VmError::UnexpectedEndOfCode);
                    }
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&code[pc..pc + 8]);
                    pc += 8;
                    self.stack.push(Value::Int(i64::from_le_bytes(bytes)));
                }
                Opcode::LoadLocal => {
                    let idx = self.read_u32(code, &mut pc)? as usize;
                    let v = *self.locals.get(idx).ok_or(VmError::LocalOutOfBounds(idx))?;
                    self.stack.push(v);
                }
                Opcode::StoreLocal => {
                    let idx = self.read_u32(code, &mut pc)? as usize;
                    if idx >= self.locals.len() {
                        return Err(VmError::LocalOutOfBounds(idx));
                    }
                    let v = self.pop()?;
                    self.locals[idx] = v;
                }
                Opcode::Add
                | Opcode::Sub
                | Opcode::Mul
                | Opcode::Div
                | Opcode::Rem
                | Opcode::Eq
                | Opcode::Ne
                | Opcode::Lt
                | Opcode::Gt
                | Opcode::Le
                | Opcode::Ge
                | Opcode::And
                | Opcode::Or
                | Opcode::BitAnd
                | Opcode::BitOr
                | Opcode::BitXor
                | Opcode::Shl
                | Opcode::Shr => {
                    let b = self.pop()?.as_int();
                    let a = self.pop()?.as_int();
                    let r = self.binop(op, a, b)?;
                    self.stack.push(Value::Int(r));
                }
                Opcode::Not | Opcode::Neg => {
                    let a = self.pop()?.as_int();
                    let r = self.unop(op, a);
                    self.stack.push(Value::Int(r));
                }
                Opcode::Jump => {
                    let target = self.read_u32(code, &mut pc)? as usize;
                    pc = target;
                }
                Opcode::JumpIf => {
                    let target = self.read_u32(code, &mut pc)? as usize;
                    let cond = self.pop()?.as_int();
                    if cond != 0 {
                        pc = target;
                    }
                }
                Opcode::Return => {
                    return self.pop();
                }
                // Not yet implemented in the MVP — surface explicitly.
                other => return Err(VmError::UnsupportedOpcode(other)),
            }
        }

        Err(VmError::UnexpectedEndOfCode)
    }

    fn binop(&self, op: Opcode, a: i64, b: i64) -> Result<i64, VmError> {
        Ok(match op {
            Opcode::Add => a.wrapping_add(b),
            Opcode::Sub => a.wrapping_sub(b),
            Opcode::Mul => a.wrapping_mul(b),
            Opcode::Div => a.checked_div(b).unwrap_or(0),
            Opcode::Rem => a.checked_rem(b).unwrap_or(0),
            Opcode::Eq => (a == b) as i64,
            Opcode::Ne => (a != b) as i64,
            Opcode::Lt => (a < b) as i64,
            Opcode::Gt => (a > b) as i64,
            Opcode::Le => (a <= b) as i64,
            Opcode::Ge => (a >= b) as i64,
            Opcode::And => ((a != 0) && (b != 0)) as i64,
            Opcode::Or => ((a != 0) || (b != 0)) as i64,
            Opcode::BitAnd => a & b,
            Opcode::BitOr => a | b,
            Opcode::BitXor => a ^ b,
            Opcode::Shl => a.wrapping_shl(b as u32),
            Opcode::Shr => a.wrapping_shr(b as u32),
            _ => unreachable!("binop called on non-binary op"),
        })
    }

    fn unop(&self, op: Opcode, a: i64) -> i64 {
        match op {
            Opcode::Not => (a == 0) as i64,
            Opcode::Neg => a.wrapping_neg(),
            _ => unreachable!("unop called on non-unary op"),
        }
    }

    fn pop(&mut self) -> ExecResult<Value> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn read_u32(&self, code: &[u8], pc: &mut usize) -> ExecResult<u32> {
        if *pc + 4 > code.len() {
            return Err(VmError::UnexpectedEndOfCode);
        }
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&code[*pc..*pc + 4]);
        *pc += 4;
        Ok(u32::from_le_bytes(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32le(v: u32) -> [u8; 4] {
        v.to_le_bytes()
    }
    fn i64le(v: i64) -> [u8; 8] {
        v.to_le_bytes()
    }

    #[test]
    fn run_arithmetic_expression() {
        // (3 + 4) * 2 == 14
        let mut bc = Vec::new();
        bc.push(Opcode::LoadConst as u8);
        bc.extend_from_slice(&i64le(3));
        bc.push(Opcode::LoadConst as u8);
        bc.extend_from_slice(&i64le(4));
        bc.push(Opcode::Add as u8);
        bc.push(Opcode::LoadConst as u8);
        bc.extend_from_slice(&i64le(2));
        bc.push(Opcode::Mul as u8);
        bc.push(Opcode::Return as u8);

        let mut vm = Vm::new(0);
        let result = vm.run(&Chunk::new(bc)).unwrap();
        assert_eq!(result, Value::Int(14));
    }

    #[test]
    fn run_conditional_jump_skips_dead_code() {
        // result = 10; if true { goto end }; result = 99; <end> return
        // -> should return 10 (the 99 is skipped).
        let mut bc = Vec::new();
        bc.push(Opcode::LoadConst as u8);
        bc.extend_from_slice(&i64le(10));
        bc.push(Opcode::StoreLocal as u8);
        bc.extend_from_slice(&u32le(0)); // local 0 = 10
        bc.push(Opcode::LoadConst as u8);
        bc.extend_from_slice(&i64le(1)); // condition = true
        bc.push(Opcode::JumpIf as u8);
        let jif_pos = bc.len();
        bc.extend_from_slice(&u32le(0)); // patched below
        bc.push(Opcode::LoadConst as u8);
        bc.extend_from_slice(&i64le(99));
        bc.push(Opcode::StoreLocal as u8);
        bc.extend_from_slice(&u32le(0)); // local 0 = 99 (dead)
        let end_pos = bc.len();
        // patch JumpIf target
        let target_bytes = u32le(end_pos as u32);
        bc[jif_pos..jif_pos + 4].copy_from_slice(&target_bytes);
        bc.push(Opcode::LoadLocal as u8);
        bc.extend_from_slice(&u32le(0)); // load local 0
        bc.push(Opcode::Return as u8);

        let mut vm = Vm::new(1);
        let result = vm.run(&Chunk::new(bc)).unwrap();
        assert_eq!(result, Value::Int(10));
    }

    #[test]
    fn run_logical_and_short_semantics() {
        // (1 != 0) && (2 < 3) -> 1
        let mut bc = Vec::new();
        bc.push(Opcode::LoadConst as u8);
        bc.extend_from_slice(&i64le(1));
        bc.push(Opcode::LoadConst as u8);
        bc.extend_from_slice(&i64le(0));
        bc.push(Opcode::Ne as u8); // 1 != 0 -> 1
        bc.push(Opcode::LoadConst as u8);
        bc.extend_from_slice(&i64le(2));
        bc.push(Opcode::LoadConst as u8);
        bc.extend_from_slice(&i64le(3));
        bc.push(Opcode::Lt as u8); // 2 < 3 -> 1
        bc.push(Opcode::And as u8); // 1 && 1 -> 1
        bc.push(Opcode::Return as u8);

        let mut vm = Vm::new(0);
        let result = vm.run(&Chunk::new(bc)).unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn unknown_opcode_reports_error() {
        let bc = vec![0x7F]; // not a defined opcode
        let mut vm = Vm::new(0);
        let err = vm.run(&Chunk::new(bc)).unwrap_err();
        assert_eq!(err, VmError::UnknownOpcode(0x7F));
    }
}
