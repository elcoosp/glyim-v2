use glyim_diag::GlyimDiagnostic;
use glyim_span::FileId;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct CompileOutput {
    pub diagnostics: Vec<GlyimDiagnostic>,
    pub syntax_tree: Option<glyim_syntax::SyntaxNode>,
    pub def_map: Option<glyim_def_map::CrateDefMap>,
    pub typeck_result: Option<glyim_typeck::TypeckResult>,
    pub mir_bodies: Vec<Arc<glyim_mir::Body>>,
    pub ty_ctx: Option<Arc<glyim_type::TyCtx>>,
}

impl std::fmt::Debug for CompileOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompileOutput")
            .field("diagnostics", &self.diagnostics)
            .field("syntax_tree", &self.syntax_tree)
            .field("def_map", &self.def_map)
            .field("typeck_result", &self.typeck_result)
            .field("mir_bodies", &self.mir_bodies)
            .field("ty_ctx", &self.ty_ctx.as_ref().map(|_| "TyCtx"))
            .finish()
    }
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
            ty_ctx: None,
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
        let path = std::env::temp_dir().join(format!("glyim_test_{}.g", file_id.to_raw()));
        std::fs::write(&path, source).expect("failed to write temp source file for PipelineCompiler");
        db.vfs().add_file_content(&path, Arc::from(source));

        let output_path = std::env::temp_dir().join(format!("glyim_test_{}.o", file_id.to_raw()));
        let ty_ctx = db.get_ty_ctx();
        match glyim_pipeline::Pipeline::compile_file_with_artifacts(
            &mut db,
            &path,
            &*self.backend,
            &output_path,
        ) {
            Ok(artifacts) => CompileOutput {
                diagnostics: Vec::new(),
                syntax_tree: None,
                def_map: Some(artifacts.def_map),
                typeck_result: Some(artifacts.typeck_result),
                mir_bodies: artifacts.mir_bodies,
                ty_ctx: Some(artifacts.ty_ctx),
            },
            Err(diags) => CompileOutput {
                diagnostics: diags,
                syntax_tree: None,
                def_map: None,
                typeck_result: None,
                mir_bodies: Vec::new(),
                ty_ctx,
            },
        }
    }
}
