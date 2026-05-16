use super::helpers::*;
use glyim_core::primitives::*;
use glyim_mir::{AssertMessage, Operand, TerminatorKind};
use glyim_type::{TyCtxMut, TyKind};

#[test]
fn test_overflow_add_i32_panics() {
    // Build MIR: assign large values, then Assert overflow
    // This test will be implemented when Assert lowering is done.
}
