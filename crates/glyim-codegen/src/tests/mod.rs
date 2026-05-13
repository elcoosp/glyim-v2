//! Tests for glyim-codegen BytecodeBackend.
//!
//! Test plan:
//! - S07-T01: Empty function → module with Return opcode
//! - S07-T02: Integer constants → LoadConst + Add + Return
//! - S07-T03: Locals → LoadLocal + StoreLocal
//! - S07-T04: Branch → JumpIf + Jump
//! - S07-T05: generate() returns non-empty Vec<u8>
//! - S07-T06: name() returns 'bytecode'

mod backend_tests;
