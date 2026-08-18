use glyim_diag::GlyimDiagnostic;
use glyim_span::FileId;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct CompileOutput {
    pub diagnostics: Vec<GlyimDiagnostic>,
    pub syntax_tree: Option<glyim_syntax::SyntaxNode>,
    pub def_map: Option<glyim_def_map::CrateDefMap>,
    pub typeck_result: Option<glyim_typeck::TypeckResult>,
    pub mir_bodies: Vec<Arc<glyim_mir::Body>>,
    pub ty_ctx: Option<Arc<glyim_type::TyCtx>>,
    /// Path to a linked executable, populated only when compilation succeeded
    /// AND the produced object file was successfully linked (Tier 7.2). `None`
    /// otherwise (e.g. a mock backend that emits no real object, or a link
    /// failure) — in which case run-pass/run-fail strategies report "no
    /// executable produced" rather than silently running nothing.
    pub executable_path: Option<PathBuf>,
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
            .field("executable_path", &self.executable_path)
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
            executable_path: None,
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

        // Each `compile()` call writes its source to a temp file that the
        // pipeline reads back. The path must be UNIQUE per call: many tests
        // run concurrently and several reuse the same `FileId` (e.g. `1`), so
        // a path derived only from `file_id` collides, causing one thread to
        // overwrite another's source and the compiler to read the wrong
        // program ("MIR body not found"). We uniquify with a process-global
        // counter combined with the pid.
        static CALL_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let call_id = CALL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let unique_tag = format!("{}_{}", std::process::id(), call_id);

        let config = CrateConfig {
            name: format!("test_{}", file_id.to_raw()),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            opt_level: 0,
        };

        let mut db = Database::new(config);
        let path = std::env::temp_dir().join(format!("glyim_test_{}_{}.g", unique_tag, file_id.to_raw()));
        std::fs::write(&path, source).expect("failed to write temp source file for PipelineCompiler");
        db.vfs().add_file_content(&path, Arc::from(source));

        let output_path = std::env::temp_dir().join(format!("glyim_test_{}_{}.o", unique_tag, file_id.to_raw()));
        let ty_ctx = db.get_ty_ctx();
        let exe_path = output_path.with_extension("");
        match glyim_pipeline::Pipeline::compile_file_with_artifacts(
            &mut db,
            &path,
            &*self.backend,
            &output_path,
        ) {
            Ok(artifacts) => {
                // Tier 7.2: link the produced object file into a real executable
                // so run-pass/run-fail strategies can actually execute it. The
                // linker is only invoked when codegen produced a real object
                // (the production LLVM backend does; a mock backend that emits
                // nothing will fail to link and we fall back to `None`).
                let executable_path = glyim_cli::linker::invoke_linker(&output_path, &exe_path, None, None, None)
                    .ok()
                    .map(|()| exe_path.clone());
                CompileOutput {
                    diagnostics: Vec::new(),
                    syntax_tree: None,
                    def_map: Some(artifacts.def_map),
                    typeck_result: Some(artifacts.typeck_result),
                    mir_bodies: artifacts.mir_bodies,
                    ty_ctx: Some(artifacts.ty_ctx),
                    executable_path,
                }
            }
            Err(diags) => CompileOutput {
                diagnostics: diags,
                syntax_tree: None,
                def_map: None,
                typeck_result: None,
                mir_bodies: Vec::new(),
                ty_ctx,
                executable_path: None,
            },
        }
    }
}
