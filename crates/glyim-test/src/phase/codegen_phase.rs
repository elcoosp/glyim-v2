use std::sync::Arc;

pub struct CodegenTester;

impl CodegenTester {
    pub fn generate(
        backend: &dyn glyim_codegen::CodegenBackend,
        bodies: &[Arc<glyim_mir::Body>],
        output: &std::path::Path,
    ) -> glyim_diag::CompResult<Vec<u8>> {
        backend.generate(bodies, output)
    }
    pub fn generate_function(
        backend: &dyn glyim_codegen::CodegenBackend,
        body: &Arc<glyim_mir::Body>,
    ) -> glyim_diag::CompResult<Vec<u8>> {
        backend.generate_function(body)
    }
}
