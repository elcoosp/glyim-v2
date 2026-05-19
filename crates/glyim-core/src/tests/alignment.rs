use crate::abi::{ALIGN_MAX, ALIGN_MIN, validate_alignment};

#[test]
fn valid_alignments() {
    assert!(validate_alignment(1).is_ok());
    assert!(validate_alignment(2).is_ok());
    assert!(validate_alignment(4).is_ok());
    assert!(validate_alignment(8).is_ok());
    assert!(validate_alignment(16).is_ok());
    assert!(validate_alignment(ALIGN_MIN).is_ok());
    assert!(validate_alignment(ALIGN_MAX).is_ok());
}

#[test]
fn invalid_alignments() {
    assert!(validate_alignment(0).is_err());
    assert!(validate_alignment(3).is_err());
    assert!(validate_alignment(6).is_err());
    assert!(validate_alignment(12).is_err());
    assert!(validate_alignment(32).is_err());
    assert!(validate_alignment(ALIGN_MAX * 2).is_err());
}
