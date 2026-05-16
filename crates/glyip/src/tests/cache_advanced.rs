//! Advanced cache tests — edge cases and directory structure.

use crate::cache::Cache;
use tempfile::TempDir;

#[test]
fn cache_dep_dir() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");
    let dep_dir = cache.dep_dir();
    assert!(dep_dir.to_string_lossy().contains("dep"));
}

#[test]
fn cache_global_cache_dir_has_glyip() {
    let global = Cache::global_cache_dir();
    assert!(global.to_string_lossy().contains(".glyip"));
    assert!(global.to_string_lossy().contains("cache"));
}

#[test]
fn cache_needs_recompile_new_file() {
    let dir = TempDir::new().expect("temp dir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("mkdir");
    let file = src.join("main.g");
    std::fs::write(&file, "fn main() {}").expect("write");

    let cache = Cache::new(dir.path()).expect("cache");
    assert!(cache.needs_recompile(&file).expect("recompile check"));
}

#[test]
fn cache_needs_recompile_after_mark_built() {
    let dir = TempDir::new().expect("temp dir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("mkdir");
    let file = src.join("main.g");
    std::fs::write(&file, "fn main() {}").expect("write");

    let mut cache = Cache::new(dir.path()).expect("cache");
    cache.mark_built().expect("mark built");
    assert!(
        !cache
            .needs_recompile(&file)
            .expect("recompile check after mark")
    );
}

#[test]
fn cache_artifact_overwrite() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");

    cache.store_artifact("key1", b"v1").expect("store v1");
    cache.store_artifact("key1", b"v2").expect("store v2");

    let data = cache.get_artifact("key1").expect("get").expect("some");
    assert_eq!(data, b"v2");
}

#[test]
fn cache_artifact_missing_key() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");
    let result = cache.get_artifact("nonexistent").expect("get");
    assert!(result.is_none());
}

#[test]
fn cache_multiple_artifacts() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");

    cache.store_artifact("a", b"data-a").expect("store a");
    cache.store_artifact("b", b"data-b").expect("store b");
    cache.store_artifact("c", b"data-c").expect("store c");

    assert_eq!(
        cache.get_artifact("a").expect("get").expect("some"),
        b"data-a"
    );
    assert_eq!(
        cache.get_artifact("b").expect("get").expect("some"),
        b"data-b"
    );
    assert_eq!(
        cache.get_artifact("c").expect("get").expect("some"),
        b"data-c"
    );
}

#[test]
fn cache_output_binary_release() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");
    let bin = cache.output_binary("myapp", true);
    assert!(bin.to_string_lossy().contains("release"));
    assert!(bin.to_string_lossy().contains("myapp"));
}

#[test]
fn cache_clean_then_recreate() {
    let dir = TempDir::new().expect("temp dir");
    let cache = Cache::new(dir.path()).expect("cache");
    assert!(cache.target_dir().exists());

    cache.clean().expect("clean");
    assert!(!cache.target_dir().exists());

    // Recreate
    let cache2 = Cache::new(dir.path()).expect("cache2");
    assert!(cache2.target_dir().exists());
}

#[test]
fn cache_mark_built_includes_config() {
    let dir = TempDir::new().expect("temp dir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("mkdir");
    std::fs::write(src.join("main.g"), "fn main() {}").expect("write");

    // Create a Glyip.toml so it gets fingerprinted too
    let config_content = r#"[package]
name = "test"
version = "0.1.0"
edition = "2024"
"#;
    std::fs::write(dir.path().join("Glyip.toml"), config_content).expect("write config");

    let mut cache = Cache::new(dir.path()).expect("cache");
    cache.mark_built().expect("mark built");

    // Should have fingerprinted both src/main.g and Glyip.toml
    assert!(!cache.needs_rebuild().expect("rebuild"));
}
