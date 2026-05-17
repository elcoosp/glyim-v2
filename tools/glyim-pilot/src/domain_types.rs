//! Domain types shared between config and implementation modules.
//! Defined here so `config` does not depend on `applier` or `gates`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyLimits {
    pub max_file_size: usize,
    pub max_total_content: usize,
    pub max_ops_per_block: usize,
}

impl Default for ApplyLimits {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024,
            max_total_content: 50 * 1024 * 1024,
            max_ops_per_block: 100,
        }
    }
}

impl ApplyLimits {
    pub fn strict() -> Self {
        Self {
            max_file_size: 1024 * 1024,
            max_total_content: 5 * 1024 * 1024,
            max_ops_per_block: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BannedPattern {
    pub pattern: String,
    pub description: String,
}

impl BannedPattern {
    pub fn new(pattern: impl Into<String>, description: impl Into<String>) -> Self {
        Self { pattern: pattern.into(), description: description.into() }
    }
}

pub fn default_banned_patterns() -> Vec<BannedPattern> {
    vec![
        BannedPattern::new("todo!()", "`todo!()` in non-test code"),
        BannedPattern::new("unwrap()", "`.unwrap()` in non-test code"),
        BannedPattern::new("panic!()", "`panic!()` in non-test code"),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRule {
    pub from_crate: String,
    pub forbidden_dep: String,
    pub reason: String,
}

pub fn default_architecture_rules() -> Vec<DependencyRule> {
    vec![
        DependencyRule { from_crate: "glyim-frontend".into(), forbidden_dep: "glyim-type".into(), reason: "frontend must not depend on type directly".into() },
        DependencyRule { from_crate: "glyim-frontend".into(), forbidden_dep: "glyim-ir".into(), reason: "frontend must not depend on IR".into() },
        DependencyRule { from_crate: "glyim-syntax".into(), forbidden_dep: "glyim-ir".into(), reason: "syntax must not depend on IR".into() },
        DependencyRule { from_crate: "glyim-type".into(), forbidden_dep: "glyim-codegen".into(), reason: "type must not depend on codegen".into() },
    ]
}
