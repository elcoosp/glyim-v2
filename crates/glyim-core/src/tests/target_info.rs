use crate::primitives::{TargetAbi, TargetInfo};

#[test]
fn target_info_x86_64() {
    let target = TargetInfo::x86_64();
    assert_eq!(target.pointer_width(), 64);
    assert_eq!(target.pointer_size(), 8);
    assert_eq!(target.pointer_align(), 8);
    assert_eq!(target.triple, "x86_64-unknown-linux-gnu");
    assert_eq!(target.abi, TargetAbi::X86_64SystemV);
}

#[test]
fn target_info_aarch64() {
    let target = TargetInfo::aarch64();
    assert_eq!(target.pointer_width(), 64);
    assert_eq!(target.pointer_size(), 8);
    assert_eq!(target.pointer_align(), 8);
    assert_eq!(target.triple, "aarch64-unknown-linux-gnu");
    assert_eq!(target.abi, TargetAbi::AArch64AAPCS);
}

#[test]
fn target_info_default() {
    let target = TargetInfo::default();
    assert_eq!(target.pointer_width(), 64);
    assert_eq!(target.pointer_size(), 8);
    assert_eq!(target.triple, "x86_64-unknown-linux-gnu");
    assert_eq!(target.abi, TargetAbi::X86_64SystemV);
}
