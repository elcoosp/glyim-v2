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
