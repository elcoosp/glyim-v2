use crate::alloc::{Layout, LayoutError};

#[test]
fn layout_valid() {
    let layout = Layout::from_size_align(1024, 8).unwrap();
    assert_eq!(layout.size(), 1024);
    assert_eq!(layout.align(), 8);
}

#[test]
fn layout_invalid_alignment() {
    assert_eq!(
        Layout::from_size_align(1024, 3),
        Err(LayoutError::InvalidAlignment(3))
    );
}

#[test]
fn layout_size_overflow() {
    let result = Layout::from_size_align(usize::MAX, 4);
    assert!(result.is_err());
}

#[test]
fn layout_minimal() {
    let layout = Layout::from_size_align(0, 1).unwrap();
    assert_eq!(layout.size(), 0);
    assert_eq!(layout.align(), 1);
}

#[test]
fn layout_power_of_two_alignments() {
    for &align in &[1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] {
        let layout = Layout::from_size_align(align, align).unwrap();
        assert_eq!(layout.align(), align);
    }
}

#[test]
fn layout_large_valid() {
    let layout = Layout::from_size_align(1 << 20, 16).unwrap();
    assert_eq!(layout.size(), 1 << 20);
    assert_eq!(layout.align(), 16);
}

#[test]
fn layout_size_align_equals_max() {
    // size == usize::MAX - (align - 1) should be valid
    let layout = Layout::from_size_align(usize::MAX - 7, 8);
    assert!(layout.is_ok());
}
