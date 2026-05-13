use glyim_codegen::CodegenBackend;
use std::sync::Arc;
use glyim_mir::Body;
use glyim_diag::CompResult;
pub struct LlvmBackend;
impl LlvmBackend { pub fn new() -> Self { Self } }
impl CodegenBackend for LlvmBackend {
    fn name(&self) -> &'static str { "llvm" }
    fn generate(&self, _bodies: &[Arc<Body>], _output: &std::path::Path) -> CompResult<Vec<u8>> {
        Ok(Vec::new())
    }
}
