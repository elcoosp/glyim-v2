use std::path::Path;
use std::sync::Arc;
use glyim_mir::Body;
use glyim_diag::CompResult;
pub trait CodegenBackend {
    fn name(&self) -> &'static str;
    fn generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<Vec<u8>>;
}
pub struct BytecodeBackend;
impl BytecodeBackend { pub fn new() -> Self { Self } }
impl CodegenBackend for BytecodeBackend {
    fn name(&self) -> &'static str { "bytecode" }
    fn generate(&self, _bodies: &[Arc<Body>], _output: &Path) -> CompResult<Vec<u8>> { Ok(Vec::new()) }
}
