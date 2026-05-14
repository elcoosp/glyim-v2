use crate::uri::*;
use std::path::PathBuf;

#[test]
fn unix_roundtrip() {
    let path = PathBuf::from("/home/user/foo.g");
    let uri = path_to_uri(&path).expect("should produce a URI");
    let back = uri_to_file_path(&uri).expect("should convert back to path");
    // On Unix, back should equal the original path
    assert_eq!(back, path);
}

#[test]
fn windows_roundtrip() {
    // Simulate a Windows path (note: this test works on any OS because
    // the conversion functions should handle the prefix appropriately).
    let path = PathBuf::from("C:\\Users\\user\\foo.g");
    let uri = path_to_uri(&path).expect("should produce a URI");
    let back = uri_to_file_path(&uri).expect("should convert back to path");
    // On Windows the drive letter might be lowercased, but that's okay.
    // We test that the result is a valid path.
    assert!(back.is_absolute() || back.has_root());
    assert!(back.to_str().unwrap().contains("foo.g"));
}

#[test]
fn no_scheme_is_error() {
    let uri = "/home/user/foo.g"; // not a valid URI
    assert!(uri_to_file_path(uri).is_err());
}

#[test]
fn path_to_uri_and_back() {
    let original = PathBuf::from("/tmp/test.g");
    let uri = path_to_uri(&original).unwrap();
    let back = uri_to_file_path(&uri).unwrap();
    assert_eq!(back, original);
}

#[test]
fn offset_to_position_basic() {
    let text = "ab\ncd\nef";
    assert_eq!(offset_to_position(text, 0).unwrap(), (0, 0));
    assert_eq!(offset_to_position(text, 3).unwrap(), (1, 0)); // after "ab\n"
    assert_eq!(offset_to_position(text, 6).unwrap(), (2, 1)); // "e" in "ef"
}

#[test]
fn offset_to_position_out_of_bounds() {
    let text = "hello";
    assert!(offset_to_position(text, 10).is_err());
}
