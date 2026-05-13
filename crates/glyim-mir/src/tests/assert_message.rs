use crate::*;
use glyim_core::primitives::BinOp;

#[test]
fn assert_message_overflow_add() {
    let msg = AssertMessage::Overflow(BinOp::Add);
    assert!(matches!(msg, AssertMessage::Overflow(BinOp::Add)));
}

#[test]
fn assert_message_overflow_sub() {
    let msg = AssertMessage::Overflow(BinOp::Sub);
    assert!(matches!(msg, AssertMessage::Overflow(BinOp::Sub)));
}

#[test]
fn assert_message_division_by_zero() {
    let msg = AssertMessage::DivisionByZero;
    assert!(matches!(msg, AssertMessage::DivisionByZero));
}

#[test]
fn assert_message_remainder_by_zero() {
    let msg = AssertMessage::RemainderByZero;
    assert!(matches!(msg, AssertMessage::RemainderByZero));
}

#[test]
fn assert_message_bounds_check() {
    let msg = AssertMessage::BoundsCheck;
    assert!(matches!(msg, AssertMessage::BoundsCheck));
}
