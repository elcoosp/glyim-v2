use glyim_diag::GlyimDiagnostic;
use glyim_span::FileId;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct CompileOutput {
    pub diagnostics: Vec<GlyimDiagnostic>,
    pub syntax_tree: Option<glyim_syntax::SyntaxNode>,
    pub def_map: Option<glyim_def_map::CrateDefMap>,
    pub typeck_result: Option<glyim_typeck::TypeckResult>,
    pub mir_bodies: Vec<Arc<glyim_mir::Body>>,
}

pub trait TestCompiler: Send + Sync {
    fn compile(&self, source: &str, file_id: FileId, flags: &[String]) -> CompileOutput;
}

pub struct FrontendOnlyCompiler;

impl TestCompiler for FrontendOnlyCompiler {
    fn compile(&self, source: &str, file_id: FileId, _flags: &[String]) -> CompileOutput {
        tracing::info!(phase = "parse", file_id = file_id.to_raw());
        let result = glyim_frontend::parse_to_syntax(source, file_id);
        CompileOutput {
            diagnostics: result.diagnostics,
            syntax_tree: Some(result.root),
            def_map: None,
            typeck_result: None,
            mir_bodies: Vec::new(),
        }
    }
}

pub struct PipelineCompiler {
    backend: Arc<dyn glyim_codegen::CodegenBackend + Send + Sync>,
}

impl PipelineCompiler {
    pub fn new(backend: Arc<dyn glyim_codegen::CodegenBackend + Send + Sync>) -> Self {
        Self { backend }
    }
}

impl TestCompiler for PipelineCompiler {
    fn compile(&self, source: &str, file_id: FileId, _flags: &[String]) -> CompileOutput {
        use glyim_db::{CrateConfig, Database};

        tracing::info!(phase = "full-pipeline", file_id = file_id.to_raw());

        let config = CrateConfig {
            name: format!("test_{}", file_id.to_raw()),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            opt_level: 0,
        };

        let mut db = Database::new(config);
        let path = PathBuf::from(format!("test_{}.g", file_id.to_raw()));
        db.vfs().add_file_content(&path, Arc::from(source));

        match glyim_pipeline::Pipeline::compile_file(&mut db, &path, &*self.backend) {
            Ok(()) => CompileOutput {
                diagnostics: Vec::new(),
                syntax_tree: None,
                def_map: None,
                typeck_result: None,
                mir_bodies: Vec::new(),
            },
            Err(diags) => CompileOutput {
                diagnostics: diags,
                syntax_tree: None,
                def_map: None,
                typeck_result: None,
                mir_bodies: Vec::new(),
            },
        }
    }
}
