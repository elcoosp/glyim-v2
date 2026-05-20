//! Tests for artifact caching and incremental rebuild — S12-T03.

use crate::cache::Cache;
use std::fs;
use tempfile::TempDir;

fn make_project_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();

    fs::write(
        dir.path().join("Glyip.toml"),
        "[package]\nname = \"test-proj\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();

    fs::write(src_dir.join("main.g"), "fn main() {}\n").unwrap();
    dir
}

#[test]
fn store_and_retrieve_artifact() {
    // S12-T03: Cached artifact reused when fingerprint unchanged.
    let dir = make_project_dir();
    let cache = Cache::new(dir.path()).unwrap();

    let key = "test-module-hash123";
    let data = b"bytecode-content-here";

    let stored_path = cache.store_artifact(key, data).unwrap();
    assert!(stored_path.exists(), "stored artifact file should exist");

    let retrieved = cache.get_artifact(key).unwrap();
    assert!(retrieved.is_some(), "artifact should be found");
    assert_eq!(
        retrieved.unwrap(),
        data.to_vec(),
        "retrieved content should match stored content"
    );
}

#[test]
fn artifact_reused_when_fingerprint_unchanged() {
    // S12-T03: Build, mark fingerprints, verify rebuild is skipped.
    let dir = make_project_dir();
    let mut cache = Cache::new(dir.path()).unwrap();

    let needs_build_before = cache.needs_rebuild().unwrap();
    assert!(needs_build_before, "first build should need recompilation");

    cache.mark_built().unwrap();

    let needs_build_after = cache.needs_rebuild().unwrap();
    assert!(
        !needs_build_after,
        "should not need rebuild after marking built"
    );
}

#[test]
fn needs_recompile_after_change() {
    let dir = make_project_dir();
    let mut cache = Cache::new(dir.path()).unwrap();

    let main_g = dir.path().join("src/main.g");

    cache.mark_built().unwrap();

    fs::write(&main_g, "fn main() { 42 }\n").unwrap();

    let needs = cache.needs_recompile(&main_g).unwrap();
    assert!(needs, "should need recompile after file change");

    let needs_rebuild = cache.needs_rebuild().unwrap();
    assert!(needs_rebuild, "should need rebuild after file change");
}

#[test]
fn store_and_retrieve_multiple_artifacts() {
    let dir = make_project_dir();
    let cache = Cache::new(dir.path()).unwrap();

    cache.store_artifact("mod-a", b"content-a").unwrap();
    cache.store_artifact("mod-b", b"content-b").unwrap();
    cache.store_artifact("mod-c", b"content-c").unwrap();

    assert_eq!(
        cache.get_artifact("mod-a").unwrap().unwrap(),
        b"content-a".to_vec()
    );
    assert_eq!(
        cache.get_artifact("mod-b").unwrap().unwrap(),
        b"content-b".to_vec()
    );
    assert_eq!(
        cache.get_artifact("mod-c").unwrap().unwrap(),
        b"content-c".to_vec()
    );
}

#[test]
fn get_nonexistent_artifact() {
    let dir = make_project_dir();
    let cache = Cache::new(dir.path()).unwrap();

    let result = cache.get_artifact("does-not-exist").unwrap();
    assert!(result.is_none(), "non-existent artifact should return None");
}

#[test]
fn clean_removes_artifacts() {
    let dir = make_project_dir();
    let cache = Cache::new(dir.path()).unwrap();

    cache.store_artifact("to-delete", b"gone").unwrap();
    assert!(cache.get_artifact("to-delete").unwrap().is_some());

    cache.clean().unwrap();
    assert!(
        !cache.target_dir().exists(),
        "target directory should be removed after clean"
    );
}

#[test]
fn cache_output_dir_debug_release() {
    let dir = make_project_dir();
    let cache = Cache::new(dir.path()).unwrap();

    let debug_dir = cache.debug_dir();
    let release_dir = cache.release_dir();

    assert!(debug_dir.to_string_lossy().contains("debug"));
    assert!(release_dir.to_string_lossy().contains("release"));
    assert_eq!(cache.output_dir(false), debug_dir);
    assert_eq!(cache.output_dir(true), release_dir);
}

#[test]
fn output_binary_path() {
    let dir = make_project_dir();
    let cache = Cache::new(dir.path()).unwrap();

    let debug_bin = cache.output_binary("my-app", false);
    let release_bin = cache.output_binary("my-app", true);

    assert!(debug_bin.to_string_lossy().contains("my-app"));
    assert!(release_bin.to_string_lossy().contains("my-app"));
    assert!(debug_bin.to_string_lossy().contains("debug"));
    assert!(release_bin.to_string_lossy().contains("release"));
}
