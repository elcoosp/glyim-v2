//! Advanced fingerprint tests — edge cases.

use crate::fingerprint::{Fingerprint, FingerprintStore};
use tempfile::TempDir;

#[test]
fn fingerprint_empty_content() {
    let fp = Fingerprint::from_content(b"");
    assert!(!fp.hash.is_empty());
    assert_eq!(fp.size, 0);
    assert_eq!(fp.mtime, 0);
}

#[test]
fn fingerprint_large_content() {
    let data = vec![0xAB_u8; 1024 * 1024]; // 1 MiB
    let fp = Fingerprint::from_content(&data);
    assert!(!fp.hash.is_empty());
    assert_eq!(fp.size, 1024 * 1024);
}

#[test]
fn fingerprint_binary_content() {
    let data: Vec<u8> = (0..=255).collect();
    let fp = Fingerprint::from_content(&data);
    assert!(!fp.hash.is_empty());
    assert_eq!(fp.size, 256);
}

#[test]
fn fingerprint_unicode_content() {
    let content = "fn main() { println!(\"日本語テスト 🦀\"); }";
    let fp = Fingerprint::from_content(content.as_bytes());
    assert!(!fp.hash.is_empty());
    assert_eq!(fp.size, content.len() as u64);
}

#[test]
fn fingerprint_store_len_and_is_empty() {
    let mut store = FingerprintStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);

    let dir = TempDir::new().expect("temp dir");
    let file_path = dir.path().join("test.g");
    std::fs::write(&file_path, "fn main() {}").expect("write");

    store.update(&file_path).expect("update");
    assert!(!store.is_empty());
    assert_eq!(store.len(), 1);
}

#[test]
fn fingerprint_store_update_overwrites() {
    let dir = TempDir::new().expect("temp dir");
    let file_path = dir.path().join("test.g");
    std::fs::write(&file_path, "version 1").expect("write v1");

    let mut store = FingerprintStore::new();
    store.update(&file_path).expect("update v1");
    assert_eq!(store.len(), 1);

    // Overwrite with new content
    std::fs::write(&file_path, "version 2 with more content").expect("write v2");
    store.update(&file_path).expect("update v2");
    assert_eq!(store.len(), 1); // still 1, overwritten

    // Should not show as changed now
    assert!(!store.has_changed(&file_path).expect("check"));
}

#[test]
fn fingerprint_store_has_any_changed_no_dir() {
    let store = FingerprintStore::new();
    let result = store.has_any_changed(std::path::Path::new("/nonexistent"), "g");
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[test]
fn fingerprint_store_update_all_no_dir() {
    let mut store = FingerprintStore::new();
    let result = store.update_all(std::path::Path::new("/nonexistent"), "g");
    assert!(result.is_ok());
}

#[test]
fn fingerprint_store_multiple_files() {
    let dir = TempDir::new().expect("temp dir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("mkdir");

    std::fs::write(src.join("a.g"), "fn a() {}").expect("write a");
    std::fs::write(src.join("b.g"), "fn b() {}").expect("write b");
    std::fs::write(src.join("c.g"), "fn c() {}").expect("write c");

    let mut store = FingerprintStore::new();
    store.update_all(dir.path(), "g").expect("update all");
    assert_eq!(store.len(), 3);
}

#[test]
fn fingerprint_store_only_tracks_matching_extension() {
    let dir = TempDir::new().expect("temp dir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("mkdir");

    std::fs::write(src.join("code.g"), "fn code() {}").expect("write g");
    std::fs::write(src.join("data.txt"), "not a source file").expect("write txt");
    std::fs::write(src.join("readme.md"), "# Readme").expect("write md");

    let mut store = FingerprintStore::new();
    store.update_all(dir.path(), "g").expect("update all");
    assert_eq!(store.len(), 1); // Only the .g file
}

#[test]
fn fingerprint_matches_self() {
    let fp = Fingerprint::from_content(b"test");
    assert!(fp.matches(&fp));
}

#[test]
fn fingerprint_store_save_load_empty() {
    let dir = TempDir::new().expect("temp dir");
    let store = FingerprintStore::new();
    store.save_to_dir(dir.path()).expect("save empty");

    let loaded = FingerprintStore::load_from_dir(dir.path()).expect("load");
    assert!(loaded.is_empty());
}
