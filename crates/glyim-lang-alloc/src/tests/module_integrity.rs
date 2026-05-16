//! Tests for the structural integrity of alloc library .g files.

use crate::alloc_source;

#[test]
fn vec_defines_methods() {
    let src = alloc_source("vec").unwrap();
    assert!(src.contains("fn new"), "vec.g should define new");
    assert!(src.contains("fn push"), "vec.g should define push");
    assert!(src.contains("fn pop"), "vec.g should define pop");
    assert!(src.contains("fn len"), "vec.g should define len");
    assert!(src.contains("fn is_empty"), "vec.g should define is_empty");
    assert!(src.contains("fn as_slice"), "vec.g should define as_slice");
    assert!(
        src.contains("fn extend_from_slice"),
        "vec.g should define extend_from_slice"
    );
    assert!(
        src.contains("impl<T> Default for Vec<T>"),
        "vec.g should impl Default"
    );
    assert!(
        src.contains("impl<T> FromIterator<T> for Vec<T>"),
        "vec.g should impl FromIterator"
    );
    assert!(
        src.contains("impl<T> Index<usize> for Vec<T>"),
        "vec.g should impl Index"
    );
}

#[test]
fn string_defines_methods() {
    let src = alloc_source("string").unwrap();
    assert!(src.contains("fn new"), "string.g should define new");
    assert!(
        src.contains("fn push_str"),
        "string.g should define push_str"
    );
    assert!(src.contains("fn as_str"), "string.g should define as_str");
    assert!(src.contains("fn len"), "string.g should define len");
    assert!(
        src.contains("fn is_empty"),
        "string.g should define is_empty"
    );
    assert!(
        src.contains("impl Default for String"),
        "string.g should impl Default"
    );
}

#[test]
fn boxed_defines_methods() {
    let src = alloc_source("boxed").unwrap();
    assert!(src.contains("fn new"), "boxed.g should define new");
    assert!(
        src.contains("impl<T> Deref for Box<T>"),
        "boxed.g should impl Deref"
    );
    assert!(
        src.contains("impl<T> DerefMut for Box<T>"),
        "boxed.g should impl DerefMut"
    );
    assert!(
        src.contains("impl<T> Drop for Box<T>"),
        "boxed.g should impl Drop"
    );
}

#[test]
fn rc_defines_methods() {
    let src = alloc_source("rc").unwrap();
    assert!(src.contains("fn new"), "rc.g should define new");
    assert!(
        src.contains("fn strong_count"),
        "rc.g should define strong_count"
    );
    assert!(
        src.contains("impl<T> Clone for Rc<T>"),
        "rc.g should impl Clone"
    );
    assert!(
        src.contains("impl<T> Deref for Rc<T>"),
        "rc.g should impl Deref"
    );
    assert!(
        src.contains("impl<T> Drop for Rc<T>"),
        "rc.g should impl Drop"
    );
}

#[test]
fn alloc_defines_types() {
    let src = alloc_source("alloc").unwrap();
    assert!(
        src.contains("struct Layout"),
        "alloc.g should define Layout"
    );
    assert!(
        src.contains("fn from_size_align"),
        "alloc.g should define from_size_align"
    );
    assert!(
        src.contains("fn size"),
        "alloc.g should define Layout::size"
    );
    assert!(
        src.contains("fn align"),
        "alloc.g should define Layout::align"
    );
    assert!(
        src.contains("enum LayoutError"),
        "alloc.g should define LayoutError"
    );
    assert!(
        src.contains("trait GlobalAlloc"),
        "alloc.g should define GlobalAlloc trait"
    );
}

#[test]
fn raw_vec_defines_methods() {
    let src = alloc_source("raw_vec").unwrap();
    assert!(src.contains("fn new"), "raw_vec.g should define new");
    assert!(
        src.contains("fn reserve"),
        "raw_vec.g should define reserve"
    );
    assert!(src.contains("fn as_ptr"), "raw_vec.g should define as_ptr");
    assert!(
        src.contains("fn as_mut_ptr"),
        "raw_vec.g should define as_mut_ptr"
    );
    assert!(
        src.contains("fn capacity"),
        "raw_vec.g should define capacity"
    );
    assert!(
        src.contains("impl<T> Default for RawVec<T>"),
        "raw_vec.g should impl Default"
    );
    assert!(
        src.contains("impl<T> Drop for RawVec<T>"),
        "raw_vec.g should impl Drop"
    );
}
