//! Tests for dependency resolution.

use crate::config::*;
use crate::dep::*;
use crate::error::GlyipError;
use crate::lockfile::{CrateSource, LockedCrate, Lockfile};
use std::collections::{BTreeMap, HashMap};
use tempfile::TempDir;

fn make_simple_config(name: &str, deps: BTreeMap<String, Dependency>) -> GlyipToml {
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
        dev_dependencies: BTreeMap::new(),
    }
}

#[test]
fn resolve_no_dependencies() {
    let config = make_simple_config("empty", BTreeMap::new());
    let resolver = DependencyResolver::new_no_index();
    let dir = TempDir::new().expect("temp dir");

    let lockfile = resolver.resolve(&config, dir.path()).expect("resolve");
    assert!(lockfile.is_empty());
}

#[test]
fn resolve_path_dependency() {
    let dir = TempDir::new().expect("temp dir");

    let dep_dir = dir.path().join("my-dep");
    std::fs::create_dir_all(dep_dir.join("src")).expect("mkdir dep");
    let dep_config = GlyipToml {
        package: PackageConfig {
            name: "my-dep".to_string(),
            version: "0.2.0".to_string(),
            edition: "2024".to_string(),
            authors: Vec::new(),
            description: None,
            bin: None,
            lib: None,
        },
        dependencies: BTreeMap::new(),
        dev_dependencies: BTreeMap::new(),
    };
    dep_config.write_to_dir(&dep_dir).expect("write dep config");

    let mut deps = BTreeMap::new();
    deps.insert(
        "my-dep".to_string(),
        Dependency::Detailed(DependencyDetail {
            version: None,
            path: Some(dep_dir.clone()),
            git: None,
            branch: None,
            tag: None,
            rev: None,
        }),
    );
    let config = make_simple_config("main", deps);

    let resolver = DependencyResolver::new_no_index();
    let lockfile = resolver.resolve(&config, dir.path()).expect("resolve");

    assert_eq!(lockfile.len(), 1);
    let locked = lockfile.get_crate("my-dep", "0.2.0").expect("find dep");
    assert!(matches!(locked.source, CrateSource::Path { .. }));
}

#[test]
fn resolve_registry_dependency() {
    let mut index = CrateIndex::new();
    index.insert(IndexEntry {
        name: "serde".to_string(),
        versions: vec!["1.0.100".to_string(), "1.0.50".to_string()],
        checksums: {
            let mut m = HashMap::new();
            m.insert("1.0.100".to_string(), "sha256:aaaa".to_string());
            m
        },
    });

    let mut deps = BTreeMap::new();
    deps.insert("serde".to_string(), Dependency::Simple("1.0".to_string()));
    let config = make_simple_config("main", deps);

    let resolver = DependencyResolver::new(index);
    let dir = TempDir::new().expect("temp dir");
    let lockfile = resolver.resolve(&config, dir.path()).expect("resolve");

    assert_eq!(lockfile.len(), 1);
    let locked = lockfile.get_crate("serde", "1.0.100").expect("find serde");
    assert!(matches!(locked.source, CrateSource::Registry { .. }));
}

#[test]
fn resolve_missing_dependency() {
    let config = make_simple_config("main", {
        let mut m = BTreeMap::new();
        m.insert(
            "nonexistent".to_string(),
            Dependency::Simple("1.0".to_string()),
        );
        m
    });

    let resolver = DependencyResolver::new_no_index();
    let dir = TempDir::new().expect("temp dir");
    let result = resolver.resolve(&config, dir.path());
    assert!(result.is_err());
}

#[test]
fn index_resolve_latest_version() {
    let mut index = CrateIndex::new();
    index.insert(IndexEntry {
        name: "foo".to_string(),
        versions: vec![
            "2.0.0".to_string(),
            "1.5.0".to_string(),
            "1.0.0".to_string(),
        ],
        checksums: HashMap::new(),
    });

    let version = index.resolve_version("foo", None).expect("version");
    assert_eq!(version, "2.0.0");
}

#[test]
fn index_resolve_version_prefix() {
    let mut index = CrateIndex::new();
    index.insert(IndexEntry {
        name: "bar".to_string(),
        versions: vec![
            "2.0.0".to_string(),
            "1.5.0".to_string(),
            "1.0.0".to_string(),
        ],
        checksums: HashMap::new(),
    });

    let version = index.resolve_version("bar", Some("1")).expect("version");
    assert_eq!(version, "1.5.0");
}

#[test]
fn index_resolve_missing_crate() {
    let index = CrateIndex::new();
    let result = index.resolve_version("missing", Some("1.0"));
    assert!(result.is_err());
}

#[test]
fn cycle_detection() {
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
            m.insert("a".to_string(), "1.0.0".to_string());
            m
        },
    });

    let resolver = DependencyResolver::new_no_index();
    let result = resolver.detect_cycles(&lf);
    assert!(result.is_err());
    if let Err(GlyipError::DependencyCycle(cycle)) = result {
        assert!(cycle.contains(&"a".to_string()) || cycle.contains(&"b".to_string()));
    } else {
        panic!("expected DependencyCycle error");
    }
}

#[test]
fn no_cycle_in_diamond() {
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
            m.insert("c".to_string(), "1.0.0".to_string());
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
            m.insert("d".to_string(), "1.0.0".to_string());
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
            m.insert("d".to_string(), "1.0.0".to_string());
            m
        },
    });
    lf.add_crate(LockedCrate {
        name: "d".to_string(),
        version: "1.0.0".to_string(),
        source: CrateSource::Path {
            path: "/d".to_string(),
        },
        dependencies: BTreeMap::new(),
    });

    let resolver = DependencyResolver::new_no_index();
    let result = resolver.detect_cycles(&lf);
    assert!(result.is_ok());
}
