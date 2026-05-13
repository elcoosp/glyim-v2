//! LLVM Backend using inkwell.

use glyim_codegen::CodegenBackend;
use std::sync::Arc;
use glyim_mir::Body;
use glyim_diag::CompResult;
use std::path::Path;

pub struct LlvmBackend {
    context: inkwell::context::Context,
    target_triple: String,
}

impl LlvmBackend {
    pub fn new() -> Self {
        Self {
            context: inkwell::context::Context::create(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
        }
    }
    pub fn with_target(target_triple: impl Into<String>) -> Self {
        Self {
            context: inkwell::context::Context::create(),
            target_triple: target_triple.into(),
        }
    }
}

impl CodegenBackend for LlvmBackend {
    fn name(&self) -> &'static str { "llvm" }
    fn generate(&self, _bodies: &[Arc<Body>], _output: &Path) -> CompResult<Vec<u8>> { Ok(Vec::new()) }
    fn generate_function(&self, _body: &Arc<Body>) -> CompResult<Vec<u8>> { Ok(Vec::new()) }
}

impl Default for LlvmBackend {
    fn default() -> Self { Self::new() }
}
