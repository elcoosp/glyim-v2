//! Tests for fingerprint computation and change detection.

use crate::fingerprint::{Fingerprint, FingerprintStore};
use tempfile::TempDir;

#[test]
fn fingerprint_from_content() {
    let fp = Fingerprint::from_content(b"hello world");
    assert!(!fp.hash.is_empty());
    assert_eq!(fp.size, 11);
}

#[test]
fn fingerprint_deterministic() {
    let fp1 = Fingerprint::from_content(b"test content");
    let fp2 = Fingerprint::from_content(b"test content");
    assert_eq!(fp1.hash, fp2.hash);
}

#[test]
fn fingerprint_differs_for_different_content() {
    let fp1 = Fingerprint::from_content(b"content A");
    let fp2 = Fingerprint::from_content(b"content B");
    assert_ne!(fp1.hash, fp2.hash);
}

#[test]
fn fingerprint_from_file() {
    let dir = TempDir::new().expect("temp dir");
    let file_path = dir.path().join("test.g");
    std::fs::write(&file_path, "fn main() {}").expect("write");

    let fp = Fingerprint::from_file(&file_path).expect("fingerprint");
    assert!(!fp.hash.is_empty());
    assert_eq!(fp.size, 12);
}

#[test]
fn fingerprint_matches_same_content() {
    let fp1 = Fingerprint::from_content(b"same");
    let fp2 = Fingerprint::from_content(b"same");
    assert!(fp1.matches(&fp2));
}

#[test]
fn fingerprint_no_match_different_content() {
    let fp1 = Fingerprint::from_content(b"aaa");
    let fp2 = Fingerprint::from_content(b"bbb");
    assert!(!fp1.matches(&fp2));
}

#[test]
fn fingerprint_store_update_and_check() {
    let dir = TempDir::new().expect("temp dir");
    let file_path = dir.path().join("src/main.g");
    std::fs::create_dir_all(dir.path().join("src")).expect("mkdir");
    std::fs::write(&file_path, "fn main() {}").expect("write");

    let mut store = FingerprintStore::new();
    assert!(store.has_changed(&file_path).expect("check"));

    store.update(&file_path).expect("update");
    assert!(!store.has_changed(&file_path).expect("check after update"));

    std::fs::write(&file_path, "fn main() { /* changed */ }").expect("write modified");
    assert!(store.has_changed(&file_path).expect("check after modify"));
}

#[test]
fn fingerprint_store_save_and_load() {
    let dir = TempDir::new().expect("temp dir");
    let file_path = dir.path().join("src/main.g");
    std::fs::create_dir_all(dir.path().join("src")).expect("mkdir");
    std::fs::write(&file_path, "fn main() {}").expect("write");

    let mut store = FingerprintStore::new();
    store.update(&file_path).expect("update");
    store.save_to_dir(dir.path()).expect("save");

    let loaded = FingerprintStore::load_from_dir(dir.path()).expect("load");
    assert!(!loaded.has_changed(&file_path).expect("check loaded"));
}

#[test]
fn fingerprint_store_has_any_changed() {
    let dir = TempDir::new().expect("temp dir");
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("mkdir");

    let file_a = src_dir.join("a.g");
    let file_b = src_dir.join("b.g");
    std::fs::write(&file_a, "fn a() {}").expect("write a");
    std::fs::write(&file_b, "fn b() {}").expect("write b");

    let mut store = FingerprintStore::new();
    assert!(store.has_any_changed(dir.path(), "g").expect("check"));

    store.update_all(dir.path(), "g").expect("update all");
    assert!(
        !store
            .has_any_changed(dir.path(), "g")
            .expect("check after update")
    );

    std::fs::write(&file_a, "fn a_modified() {}").expect("modify a");
    assert!(
        store
            .has_any_changed(dir.path(), "g")
            .expect("check after modify")
    );
}
