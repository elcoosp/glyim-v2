//! Advanced dependency resolution tests.

use crate::config::*;
use crate::dep::*;
use crate::lockfile::{CrateSource, LockedCrate, Lockfile};
use std::collections::{BTreeMap, HashMap};
use tempfile::TempDir;

fn make_config_with_deps(
    name: &str,
    deps: BTreeMap<String, Dependency>,
    dev_deps: BTreeMap<String, Dependency>,
) -> GlyipToml {
    GlyipToml {
        package: PackageConfig {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            edition: "2024".to_string(),
            authors: Vec::new(),
            description: None,
            bin: None,
            lib: None,
        },
        dependencies: deps,
        dev_dependencies: dev_deps,
    }
}

#[test]
fn resolve_dev_dependencies() {
    let dir = TempDir::new().expect("temp dir");
    let mut index = CrateIndex::new();
    index.insert(IndexEntry {
        name: "test-util".to_string(),
        versions: vec!["0.1.0".to_string()],
        checksums: HashMap::new(),
    });

    let dev_deps = {
        let mut m = BTreeMap::new();
        m.insert(
            "test-util".to_string(),
            Dependency::Simple("0.1".to_string()),
        );
        m
    };

    let config = make_config_with_deps("main", BTreeMap::new(), dev_deps);
    let resolver = DependencyResolver::new(index);
    let lockfile = resolver.resolve(&config, dir.path()).expect("resolve");

    assert_eq!(lockfile.len(), 1);
    let locked = lockfile.get_crate("test-util", "0.1.0").expect("find");
    assert_eq!(locked.name, "test-util");
}

#[test]
fn resolve_multiple_dependencies() {
    let dir = TempDir::new().expect("temp dir");
    let mut index = CrateIndex::new();
    index.insert(IndexEntry {
        name: "dep-a".to_string(),
        versions: vec!["1.0.0".to_string()],
        checksums: {
            let mut m = HashMap::new();
            m.insert("1.0.0".to_string(), "sha256:aaa".to_string());
            m
        },
    });
    index.insert(IndexEntry {
        name: "dep-b".to_string(),
        versions: vec!["2.0.0".to_string()],
        checksums: {
            let mut m = HashMap::new();
            m.insert("2.0.0".to_string(), "sha256:bbb".to_string());
            m
        },
    });

    let deps = {
        let mut m = BTreeMap::new();
        m.insert("dep-a".to_string(), Dependency::Simple("1.0".to_string()));
        m.insert("dep-b".to_string(), Dependency::Simple("2.0".to_string()));
        m
    };

    let config = make_config_with_deps("main", deps, BTreeMap::new());
    let resolver = DependencyResolver::new(index);
    let lockfile = resolver.resolve(&config, dir.path()).expect("resolve");

    assert_eq!(lockfile.len(), 2);
}

#[test]
fn resolve_deduplicates_same_dep() {
    let dir = TempDir::new().expect("temp dir");
    let mut index = CrateIndex::new();
    index.insert(IndexEntry {
        name: "shared".to_string(),
        versions: vec!["1.0.0".to_string()],
        checksums: HashMap::new(),
    });

    // Both dependencies and dev_dependencies list "shared"
    let deps = {
        let mut m = BTreeMap::new();
        m.insert("shared".to_string(), Dependency::Simple("1.0".to_string()));
        m
    };
    let dev_deps = {
        let mut m = BTreeMap::new();
        m.insert("shared".to_string(), Dependency::Simple("1.0".to_string()));
        m
    };

    let config = make_config_with_deps("main", deps, dev_deps);
    let resolver = DependencyResolver::new(index);
    let lockfile = resolver.resolve(&config, dir.path()).expect("resolve");

    // Should only appear once
    assert_eq!(lockfile.len(), 1);
}

#[test]
fn crate_index_insert_and_get() {
    let mut index = CrateIndex::new();
    index.insert(IndexEntry {
        name: "mylib".to_string(),
        versions: vec!["3.0.0".to_string()],
        checksums: HashMap::new(),
    });

    let entry = index.get("mylib").expect("find");
    assert_eq!(entry.name, "mylib");
    assert_eq!(entry.versions, vec!["3.0.0".to_string()]);
}

#[test]
fn crate_index_missing_entry() {
    let index = CrateIndex::new();
    assert!(index.get("missing").is_none());
}

#[test]
fn resolve_version_no_match_uses_latest() {
    let mut index = CrateIndex::new();
    index.insert(IndexEntry {
        name: "foo".to_string(),
        versions: vec!["5.0.0".to_string(), "4.0.0".to_string()],
        checksums: HashMap::new(),
    });

    // "99" doesn't match any version, should fall back to latest
    let version = index.resolve_version("foo", Some("99")).expect("version");
    assert_eq!(version, "5.0.0");
}

#[test]
fn detect_self_cycle() {
    let mut lf = Lockfile::new();
    lf.add_crate(LockedCrate {
        name: "self-cycle".to_string(),
        version: "1.0.0".to_string(),
        source: CrateSource::Path {
            path: "/sc".to_string(),
        },
        dependencies: {
            let mut m = BTreeMap::new();
            m.insert("self-cycle".to_string(), "1.0.0".to_string());
            m
        },
    });

    let resolver = DependencyResolver::new_no_index();
    let result = resolver.detect_cycles(&lf);
    assert!(result.is_err());
}

#[test]
fn detect_three_node_cycle() {
    let mut lf = Lockfile::new();
    lf.add_crate(LockedCrate {
        name: "a".to_string(),
        version: "1.0.0".to_string(),
        source: CrateSource::Path {
            path: "/a".to_string(),
        },
        dependencies: {
            let mut m = BTreeMap::new();
            m.insert("b".to_string(), "1.0.0".to_string());
            m
        },
    });
    lf.add_crate(LockedCrate {
        name: "b".to_string(),
        version: "1.0.0".to_string(),
        source: CrateSource::Path {
            path: "/b".to_string(),
        },
        dependencies: {
            let mut m = BTreeMap::new();
            m.insert("c".to_string(), "1.0.0".to_string());
            m
        },
    });
    lf.add_crate(LockedCrate {
        name: "c".to_string(),
        version: "1.0.0".to_string(),
        source: CrateSource::Path {
            path: "/c".to_string(),
        },
        dependencies: {
            let mut m = BTreeMap::new();
            m.insert("a".to_string(), "1.0.0".to_string());
            m
        },
    });

    let resolver = DependencyResolver::new_no_index();
    let result = resolver.detect_cycles(&lf);
    assert!(result.is_err());
}
