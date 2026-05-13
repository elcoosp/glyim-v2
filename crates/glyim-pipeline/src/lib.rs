use std::path::Path;
use glyim_diag::CompResult;
use glyim_db::Database;
use glyim_codegen::CodegenBackend;
pub struct Pipeline;
impl Pipeline {
    pub fn compile_file(db: &mut Database, path: &Path, backend: &dyn CodegenBackend) -> CompResult<()> {
        let _ = (db, path, backend);
        Ok(())
    }
}
