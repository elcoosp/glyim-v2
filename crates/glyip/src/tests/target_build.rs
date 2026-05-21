//! Tests for --target and --release support in build commands — S23.

use crate::cache::Cache;
use tempfile::TempDir;

#[test]
fn cache_profile_dep_dir_debug() {
    let dir = TempDir::new().unwrap();
    let cache = Cache::new(dir.path()).unwrap();
    let dep_dir = cache.profile_dep_dir(false);
    assert!(dep_dir.to_string_lossy().contains("debug"));
    assert!(dep_dir.to_string_lossy().contains("dep"));
}

#[test]
fn cache_profile_dep_dir_release() {
    let dir = TempDir::new().unwrap();
    let cache = Cache::new(dir.path()).unwrap();
    let dep_dir = cache.profile_dep_dir(true);
    assert!(dep_dir.to_string_lossy().contains("release"));
    assert!(dep_dir.to_string_lossy().contains("dep"));
}

#[test]
fn cache_output_dir_with_target() {
    let dir = TempDir::new().unwrap();
    let cache = Cache::new(dir.path()).unwrap();

    let out_debug = cache.output_dir_for_target(false, None);
    assert!(out_debug.to_string_lossy().contains("debug"));
    assert!(!out_debug.to_string_lossy().contains("x86_64"));

    let out_with_target = cache.output_dir_for_target(false, Some("x86_64-unknown-linux-gnu"));
    assert!(
        out_with_target
            .to_string_lossy()
            .contains("x86_64-unknown-linux-gnu")
    );
    assert!(out_with_target.to_string_lossy().contains("debug"));

    let out_release_with_target =
        cache.output_dir_for_target(true, Some("aarch64-unknown-linux-gnu"));
    assert!(
        out_release_with_target
            .to_string_lossy()
            .contains("aarch64-unknown-linux-gnu")
    );
    assert!(
        out_release_with_target
            .to_string_lossy()
            .contains("release")
    );
}

#[test]
fn cache_output_binary_for_target() {
    let dir = TempDir::new().unwrap();
    let cache = Cache::new(dir.path()).unwrap();

    let bin_plain = cache.output_binary_for_target("myapp", false, None);
    assert!(bin_plain.to_string_lossy().contains("debug"));
    assert!(bin_plain.to_string_lossy().contains("myapp"));
    assert!(!bin_plain.to_string_lossy().contains("x86_64"));

    let bin_targeted =
        cache.output_binary_for_target("myapp", true, Some("x86_64-unknown-linux-gnu"));
    assert!(
        bin_targeted
            .to_string_lossy()
            .contains("x86_64-unknown-linux-gnu")
    );
    assert!(bin_targeted.to_string_lossy().contains("release"));
    assert!(bin_targeted.to_string_lossy().contains("myapp"));
}

#[test]
fn cache_store_artifact_debug_profile() {
    let dir = TempDir::new().unwrap();
    let cache = Cache::new(dir.path()).unwrap();

    let path = cache
        .store_artifact_for_profile("test-key", b"debug-data", false)
        .unwrap();
    assert!(path.exists());
    assert!(path.to_string_lossy().contains("debug"));

    let data = cache
        .get_artifact_for_profile("test-key", false)
        .unwrap()
        .unwrap();
    assert_eq!(data, b"debug-data");
}

#[test]
fn cache_store_artifact_release_profile() {
    let dir = TempDir::new().unwrap();
    let cache = Cache::new(dir.path()).unwrap();

    let path = cache
        .store_artifact_for_profile("test-key", b"release-data", true)
        .unwrap();
    assert!(path.exists());
    assert!(path.to_string_lossy().contains("release"));

    let data = cache
        .get_artifact_for_profile("test-key", true)
        .unwrap()
        .unwrap();
    assert_eq!(data, b"release-data");
}

#[test]
fn cache_artifacts_in_different_profiles_are_separate() {
    let dir = TempDir::new().unwrap();
    let cache = Cache::new(dir.path()).unwrap();

    cache
        .store_artifact_for_profile("shared-key", b"debug-content", false)
        .unwrap();
    cache
        .store_artifact_for_profile("shared-key", b"release-content", true)
        .unwrap();

    let debug_data = cache
        .get_artifact_for_profile("shared-key", false)
        .unwrap()
        .unwrap();
    let release_data = cache
        .get_artifact_for_profile("shared-key", true)
        .unwrap()
        .unwrap();

    assert_eq!(debug_data, b"debug-content");
    assert_eq!(release_data, b"release-content");
}

#[test]
fn cache_get_artifact_missing_in_profile() {
    let dir = TempDir::new().unwrap();
    let cache = Cache::new(dir.path()).unwrap();

    let result = cache
        .get_artifact_for_profile("nonexistent", false)
        .unwrap();
    assert!(result.is_none());

    let result = cache.get_artifact_for_profile("nonexistent", true).unwrap();
    assert!(result.is_none());
}

#[test]
fn cache_output_dir_for_target_without_target_matches_output_dir() {
    let dir = TempDir::new().unwrap();
    let cache = Cache::new(dir.path()).unwrap();

    let plain_debug = cache.output_dir(false);
    let targeted_debug = cache.output_dir_for_target(false, None);
    assert_eq!(plain_debug, targeted_debug);

    let plain_release = cache.output_dir(true);
    let targeted_release = cache.output_dir_for_target(true, None);
    assert_eq!(plain_release, targeted_release);
}
