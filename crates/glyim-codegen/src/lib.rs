//! Abstract code generation backend.

use std::path::Path;
use std::sync::Arc;
use glyim_mir::Body;
use glyim_diag::CompResult;

pub trait CodegenBackend {
    fn name(&self) -> &'static str;
    fn generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<Vec<u8>>;
    fn generate_function(&self, body: &Arc<Body>) -> CompResult<Vec<u8>>;
}

pub struct BytecodeBackend;

impl Default for BytecodeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl BytecodeBackend {
    pub fn new() -> Self { Self }
}

impl CodegenBackend for BytecodeBackend {
    fn name(&self) -> &'static str { "bytecode" }
    fn generate(&self, _bodies: &[Arc<Body>], _output: &Path) -> CompResult<Vec<u8>> { Ok(Vec::new()) }
    fn generate_function(&self, _body: &Arc<Body>) -> CompResult<Vec<u8>> { Ok(Vec::new()) }
}
