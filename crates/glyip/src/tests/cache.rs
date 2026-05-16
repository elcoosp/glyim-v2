//! Tests for the build cache.

use crate::cache::Cache;
use tempfile::TempDir;

#[test]
fn cache_new_creates_target_dir() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");
    assert!(cache.target_dir().exists());
}

#[test]
fn cache_debug_and_release_dirs() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");

    let debug = cache.debug_dir();
    let release = cache.release_dir();
    assert!(debug.to_string_lossy().contains("debug"));
    assert!(release.to_string_lossy().contains("release"));
}

#[test]
fn cache_output_dir_debug() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");
    let out = cache.output_dir(false);
    assert!(out.to_string_lossy().contains("debug"));
}

#[test]
fn cache_output_dir_release() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");
    let out = cache.output_dir(true);
    assert!(out.to_string_lossy().contains("release"));
}

#[test]
fn cache_output_binary_path() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");
    let bin = cache.output_binary("myapp", false);
    assert!(bin.to_string_lossy().contains("myapp"));
}

#[test]
fn cache_needs_rebuild_empty_src() {
    let dir = TempDir::new().expect("temp dir");
    std::fs::create_dir_all(dir.path().join("src")).expect("mkdir");
    let cache = Cache::new(dir.path()).expect("cache");
    assert!(!cache.needs_rebuild().expect("rebuild check"));
}

#[test]
fn cache_needs_rebuild_new_file() {
    let dir = TempDir::new().expect("temp dir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("mkdir");
    std::fs::write(src.join("main.g"), "fn main() {}").expect("write");

    let cache = Cache::new(dir.path()).expect("cache");
    assert!(cache.needs_rebuild().expect("rebuild check"));
}

#[test]
fn cache_mark_built_then_no_rebuild() {
    let dir = TempDir::new().expect("temp dir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("mkdir");
    std::fs::write(src.join("main.g"), "fn main() {}").expect("write");

    let mut cache = Cache::new(dir.path()).expect("cache");
    assert!(cache.needs_rebuild().expect("rebuild check before mark"));
    cache.mark_built().expect("mark built");
    assert!(!cache.needs_rebuild().expect("rebuild check after mark"));
}

#[test]
fn cache_clean_removes_target() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");
    assert!(cache.target_dir().exists());
    cache.clean().expect("clean");
    assert!(!cache.target_dir().exists());
}

#[test]
fn cache_artifact_store_and_get() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");

    assert!(cache.get_artifact("test-key").expect("get").is_none());

    let path = cache
        .store_artifact("test-key", b"compiled data")
        .expect("store");
    assert!(path.exists());

    let data = cache.get_artifact("test-key").expect("get").expect("some");
    assert_eq!(data, b"compiled data");
}
