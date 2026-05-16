//! Tests for the lockfile (Glyip.lock) types.

use crate::lockfile::*;
use std::collections::BTreeMap;

#[test]
fn lockfile_new_is_empty() {
    let lf = Lockfile::new();
    assert_eq!(lf.version, 1);
    assert!(lf.is_empty());
    assert_eq!(lf.len(), 0);
}

#[test]
fn lockfile_add_and_get_crate() {
    let mut lf = Lockfile::new();
    let krate = LockedCrate {
        name: "serde".to_string(),
        version: "1.0.0".to_string(),
        source: CrateSource::Registry {
            url: "https://index.glyim.dev".to_string(),
            checksum: "abc123".to_string(),
        },
        dependencies: BTreeMap::new(),
    };
    lf.add_crate(krate.clone());
    assert_eq!(lf.len(), 1);

    let got = lf.get_crate("serde", "1.0.0").expect("crate");
    assert_eq!(got.name, "serde");
}

#[test]
fn lockfile_crates_iterator() {
    let mut lf = Lockfile::new();
    lf.add_crate(LockedCrate {
        name: "a".to_string(),
        version: "1.0.0".to_string(),
        source: CrateSource::Path {
            path: "/a".to_string(),
        },
        dependencies: BTreeMap::new(),
    });
    lf.add_crate(LockedCrate {
        name: "b".to_string(),
        version: "2.0.0".to_string(),
        source: CrateSource::Git {
            url: "https://git.example.com/b".to_string(),
            rev: Some("deadbeef".to_string()),
            branch: None,
            tag: None,
        },
        dependencies: BTreeMap::new(),
    });

    let crates: Vec<_> = lf.crates().collect();
    assert_eq!(crates.len(), 2);
}

#[test]
fn lockfile_roundtrip() {
    let mut lf = Lockfile::new();
    lf.add_crate(LockedCrate {
        name: "test-crate".to_string(),
        version: "0.5.0".to_string(),
        source: CrateSource::Registry {
            url: "https://index.glyim.dev".to_string(),
            checksum: "sha256:abcdef".to_string(),
        },
        dependencies: {
            let mut m = BTreeMap::new();
            m.insert("core".to_string(), "1.0.0".to_string());
            m
        },
    });

    let serialized = toml::to_string_pretty(&lf).expect("serialize");
    let reparsed = Lockfile::parse(&serialized).expect("re-parse");
    assert_eq!(lf, reparsed);
}

#[test]
fn lockfile_write_and_read() {
    let dir = tempfile::TempDir::new().expect("temp dir");

    let mut lf = Lockfile::new();
    lf.add_crate(LockedCrate {
        name: "example".to_string(),
        version: "3.0.0".to_string(),
        source: CrateSource::Path {
            path: "../example".to_string(),
        },
        dependencies: BTreeMap::new(),
    });

    lf.write_to_dir(dir.path()).expect("write");

    let loaded = Lockfile::read_from_dir(dir.path()).expect("read");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded.version, 1);
}

#[test]
fn lockfile_read_missing_dir_returns_empty() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let loaded = Lockfile::read_from_dir(dir.path()).expect("read missing");
    assert!(loaded.is_empty());
}

#[test]
fn crate_source_serialization() {
    let path_src = CrateSource::Path {
        path: "/local".to_string(),
    };
    let git_src = CrateSource::Git {
        url: "https://git.example.com".to_string(),
        rev: Some("abc".to_string()),
        branch: Some("main".to_string()),
        tag: None,
    };
    let reg_src = CrateSource::Registry {
        url: "https://index.glyim.dev".to_string(),
        checksum: "sha256:1234".to_string(),
    };

    for src in &[&path_src, &git_src, &reg_src] {
        let ser = toml::to_string(src).expect("serialize source");
        let de: CrateSource = toml::from_str(&ser).expect("deserialize source");
        assert_eq!(src, &&de);
    }
}
