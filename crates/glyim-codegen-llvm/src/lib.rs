use glyim_codegen::CodegenBackend;
use glyim_core::Interner;
use glyim_core::TargetInfo;
use glyim_diag::{CompResult, GlyimDiagnostic};
use glyim_mir::Body;
use glyim_span::FileId;
use glyim_type::TyCtx;
use glyim_type::TyCtxMut;
use inkwell::context::Context;
use inkwell::targets::{InitializationConfig, Target, TargetTriple};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

mod abi;
mod debug;
mod lower;
mod passes;
mod types;

pub struct LlvmBackend {
    context: Context,
    target_triple: String,
    ty_ctx: Option<TyCtx>,
    target_info: TargetInfo,
    debug_info: bool,
    source_map: HashMap<FileId, (String, String)>,
    opt_level: u8,
    opt_for_size: bool,
}

impl Default for LlvmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl LlvmBackend {
    pub fn new() -> Self {
        Target::initialize_all(&InitializationConfig::default());
        let default_ctx = TyCtxMut::new(Interner::default()).freeze();
        let target_info = TargetInfo::default();
        Self {
            context: Context::create(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            ty_ctx: Some(default_ctx),
            target_info,
            debug_info: false,
            source_map: HashMap::new(),
            opt_level: 0,
            opt_for_size: false,
        }
    }

    pub fn with_target(target_triple: impl Into<String>) -> Self {
        Target::initialize_all(&InitializationConfig::default());
        let default_ctx = TyCtxMut::new(Interner::default()).freeze();
        let triple = target_triple.into();
        let target_info = TargetInfo::default();
        Self {
            context: Context::create(),
            target_triple: triple,
            ty_ctx: Some(default_ctx),
            target_info,
            debug_info: false,
            source_map: HashMap::new(),
            opt_level: 0,
            opt_for_size: false,
        }
    }

    pub fn with_ty_ctx(mut self, ctx: TyCtx) -> Self {
        self.ty_ctx = Some(ctx);
        self
    }

    pub fn with_debug_info(mut self, enable: bool) -> Self {
        self.debug_info = enable;
        self
    }

    pub fn with_source_map(mut self, map: HashMap<FileId, (String, String)>) -> Self {
        self.source_map = map;
        self
    }

    pub(crate) fn run_passes_on_module<'ctx>(
        &self,
        module: &inkwell::module::Module<'ctx>,
        target_machine: &inkwell::targets::TargetMachine,
    ) -> Result<(), String> {
        crate::passes::run_llvm_passes(module, target_machine, self.opt_level, self.opt_for_size)
    }

    #[allow(dead_code)] // Used in tests
    pub(crate) fn lower_body_to_module<'ctx>(
        &self,
        context: &'ctx Context,
        body: &Body,
    ) -> CompResult<inkwell::module::Module<'ctx>> {
        let module = context.create_module("test_module");
        let triple = inkwell::targets::TargetTriple::create(&self.target_triple);
        module.set_triple(&triple);
        let ty_ctx = self
            .ty_ctx
            .as_ref()
            .ok_or_else(|| vec![GlyimDiagnostic::internal_error("no TyCtx available")])?;
        crate::lower::lower_body(
            context,
            &module,
            body,
            self.target_info.clone(),
            ty_ctx,
            self.debug_info,
            self.source_map.clone(),
        )?;
        Ok(module)
    }

    /// Generate LLVM IR for a single body as a string.
    ///
    /// Useful for testing type lowering without needing to write object files.
    #[allow(dead_code)] // Used in tests
    pub(crate) fn generate_ir(&self, body: &Body) -> CompResult<String> {
        let context = Context::create();
        let module = self.lower_body_to_module(&context, body)?;
        Ok(module.print_to_string().to_string())
    }
}

impl CodegenBackend for LlvmBackend {
    fn name(&self) -> &'static str {
        "llvm"
    }

    fn generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<()> {
        let context = &self.context;
        let module = context.create_module("glyim_module");
        let triple = TargetTriple::create(&self.target_triple);
        module.set_triple(&triple);

        let ty_ctx = self
            .ty_ctx
            .as_ref()
            .ok_or_else(|| vec![GlyimDiagnostic::internal_error("no TyCtx available")])?;

        for body in bodies.iter() {
            crate::lower::lower_body(
                context,
                &module,
                body,
                self.target_info.clone(),
                ty_ctx,
                self.debug_info,
                self.source_map.clone(),
            )?;
        }

        let target = Target::from_triple(&triple).map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!(
                "Target error: {}",
                e
            ))]
        })?;

        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::Default,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .ok_or_else(|| {
                vec![GlyimDiagnostic::internal_error(
                    "Failed to create target machine",
                )]
            })?;

        self.run_passes_on_module(&module, &target_machine)
            .map_err(|e| vec![GlyimDiagnostic::internal_error(e)])?;

        target_machine
            .write_to_file(&module, inkwell::targets::FileType::Object, output)
            .map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "Failed to write object file: {:?}",
                    e
                ))]
            })?;

        Ok(())
    }

    fn generate_function(&self, body: &Arc<Body>) -> CompResult<Vec<u8>> {
        let context = &self.context;
        let module = context.create_module("glyim_func");
        let triple = TargetTriple::create(&self.target_triple);
        module.set_triple(&triple);

        let ty_ctx = self
            .ty_ctx
            .as_ref()
            .ok_or_else(|| vec![GlyimDiagnostic::internal_error("no TyCtx available")])?;

        crate::lower::lower_body(
            context,
            &module,
            body,
            self.target_info.clone(),
            ty_ctx,
            self.debug_info,
            self.source_map.clone(),
        )?;

        let target = Target::from_triple(&triple).map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!(
                "Target error: {}",
                e
            ))]
        })?;

        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::Default,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .ok_or_else(|| {
                vec![GlyimDiagnostic::internal_error(
                    "Failed to create target machine",
                )]
            })?;

        self.run_passes_on_module(&module, &target_machine)
            .map_err(|e| vec![GlyimDiagnostic::internal_error(e)])?;

        target_machine
            .write_to_memory_buffer(&module, inkwell::targets::FileType::Object)
            .map(|buf| buf.as_slice().to_vec())
            .map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "Failed to generate object code: {:?}",
                    e
                ))]
            })
    }
}

#[cfg(test)]
mod tests;
