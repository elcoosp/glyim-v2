use crate::uri::*;
use std::path::PathBuf;

#[test]
fn unix_roundtrip() {
    let path = PathBuf::from("/home/user/foo.g");
    let uri = path_to_uri(&path).expect("should produce a URI");
    let back = uri_to_file_path(&uri).expect("should convert back to path");
    assert_eq!(back, path);
}

#[test]
fn windows_roundtrip() {
    let path = PathBuf::from("C:\\Users\\user\\foo.g");
    let uri = path_to_uri(&path).expect("should produce a URI");
    // The URI must start with file:/// and contain the drive letter.
    assert!(uri.starts_with("file:///"), "URI must start with file:///, got: {uri}");
    assert!(uri.contains("C:"), "URI must contain drive letter, got: {uri}");
    // Roundtrip back to a path; must be convertible to a str containing the filename.
    let back = uri_to_file_path(&uri).expect("should convert back to path");
    let back_str = back.to_str().expect("path must be valid UTF-8");
    assert!(back_str.contains("foo.g"), "result must contain filename, got: {back_str}");
}

#[test]
fn no_scheme_is_error() {
    let uri = "/home/user/foo.g";
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
    assert_eq!(offset_to_position(text, 3).unwrap(), (1, 0));
    assert_eq!(offset_to_position(text, 6).unwrap(), (2, 0));
    assert_eq!(offset_to_position(text, 7).unwrap(), (2, 1));
}

#[test]
fn offset_to_position_out_of_bounds() {
    let text = "hello";
    assert!(offset_to_position(text, 10).is_err());
}
