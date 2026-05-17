use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", content = "data")]
pub enum FileOp {
    #[serde(rename = "write")]
    Write { path: String, content: String },
    #[serde(rename = "replace")]
    Replace { path: String, find: String, replace: String },
    #[serde(rename = "delete")]
    Delete { path: String },
}

impl FileOp {
    pub fn path(&self) -> &str {
        match self {
            FileOp::Write { path, .. } => path,
            FileOp::Replace { path, .. } => path,
            FileOp::Delete { path } => path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedOps {
    pub ops: Vec<FileOp>,
    pub commit_message: Option<String>,
    pub incomplete: bool,
    pub done: bool,
    pub approved: bool,
}

impl ParsedOps {
    pub fn empty() -> Self {
        Self { ops: Vec::new(), commit_message: None, incomplete: false, done: false, approved: false }
    }
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty() && self.commit_message.is_none() && !self.incomplete && !self.done && !self.approved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version_positive() {
        assert!(PROTOCOL_VERSION > 0);
    }

    #[test]
    fn test_file_op_path_accessor() {
        assert_eq!(FileOp::Write { path: "a.rs".into(), content: String::new() }.path(), "a.rs");
        assert_eq!(FileOp::Delete { path: "c.rs".into() }.path(), "c.rs");
    }
}
