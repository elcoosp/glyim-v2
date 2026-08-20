#![allow(missing_docs)]
// Stylistic clippy lints suppressed crate-wide (test-noise lints).
#![allow(
    clippy::cloned_ref_to_slice_refs,
    clippy::vec_init_then_push,
    clippy::assertions_on_constants,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::manual_c_str_literals,
    clippy::doc_lazy_continuation,
    clippy::empty_line_after_doc_comments,
    clippy::manual_strip,
    clippy::needless_range_loop,
    clippy::unnecessary_cast,
    clippy::clone_on_copy,
    clippy::mutable_key_type,
    clippy::only_used_in_recursion,
    clippy::let_unit_value,
    clippy::unnecessary_literal_unwrap,
    clippy::format_in_format_args,
    clippy::permissions_set_readonly_false,
    clippy::needless_lifetimes,
    clippy::collapsible_if
)]
use glyim_codegen::CodegenBackend;
use glyim_core::TargetInfo;
use glyim_diag::{CompResult, GlyimDiagnostic};
use glyim_mir::Body;
use glyim_span::{FileId, HygieneCtx};
use glyim_type::TyCtx;
use inkwell::context::Context;
use inkwell::targets::{InitializationConfig, Target, TargetTriple};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

mod abi;
mod debug;
mod lower;
pub mod passes;
mod types;

pub struct LlvmBackend {
    context: Context,
    target_triple: String,
    ty_ctx_handle: Option<glyim_db::TyCtxHandle>,
    target_info: TargetInfo,
    debug_info: bool,
    source_map: HashMap<FileId, (String, String)>,
    opt_level: u8,
    opt_for_size: bool,
    hygiene_ctx: Option<HygieneCtx>,
    /// Link-time optimization strategy requested for this compilation
    /// (Phase 10.2). `None` is the default; `Fat` merges modules inside the
    /// compiler; `Thin` is a tracked gap (linker-driver integration).
    lto: crate::passes::LtoKind,
}

impl Default for LlvmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl LlvmBackend {
    pub fn new() -> Self {
        Target::initialize_all(&InitializationConfig::default());
        let target_info = TargetInfo::default();
        let default_ctx = glyim_type::TyCtxMut::new(glyim_core::Interner::default()).freeze();
        Self {
            context: Context::create(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            ty_ctx_handle: Some(Arc::new(std::sync::RwLock::new(Some(Arc::new(
                default_ctx,
            ))))),
            target_info,
            debug_info: false,
            source_map: HashMap::new(),
            opt_level: 0,
            opt_for_size: false,
            hygiene_ctx: None,
            lto: crate::passes::LtoKind::None,
        }
    }

    pub fn with_db(db: &glyim_db::Database) -> Self {
        Target::initialize_all(&InitializationConfig::default());
        let target_info = TargetInfo::default();
        Self {
            context: Context::create(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            ty_ctx_handle: Some(db.ty_ctx_handle()),
            target_info,
            debug_info: false,
            source_map: HashMap::new(),
            opt_level: 0,
            opt_for_size: false,
            hygiene_ctx: None,
            lto: crate::passes::LtoKind::None,
        }
    }

    pub fn with_hygiene_ctx(mut self, hygiene: HygieneCtx) -> Self {
        self.hygiene_ctx = Some(hygiene);
        self
    }

    pub fn lower_bodies_to_module<'ctx>(
        &self,
        context: &'ctx Context,
        bodies: &[Arc<Body>],
    ) -> CompResult<inkwell::module::Module<'ctx>> {
        let module = context.create_module("glyim_module");
        let triple = inkwell::targets::TargetTriple::create(&self.target_triple);
        module.set_triple(&triple);
        let ty_ctx = self
            .ty_ctx_handle
            .as_ref()
            .and_then(|h| h.read().unwrap().clone())
            .ok_or_else(|| vec![GlyimDiagnostic::internal_error("no TyCtx available")])?;
        let ty_ctx = ty_ctx.as_ref();
        for body in bodies {
            crate::lower::lower_body(
                context,
                &module,
                body,
                self.target_info.clone(),
                ty_ctx,
                self.debug_info,
                self.source_map.clone(),
                self.hygiene_ctx.clone(),
            )?;
        }
        Ok(module)
    }

    pub fn with_target(mut self, target_triple: impl Into<String>) -> Self {
        let triple = target_triple.into();
        self.target_info = TargetInfo::from_triple(&triple);
        self.target_triple = triple;
        self
    }

    pub fn with_ty_ctx_handle(mut self, handle: glyim_db::TyCtxHandle) -> Self {
        self.ty_ctx_handle = Some(handle);
        self
    }

    pub fn with_ty_ctx(mut self, ctx: TyCtx) -> Self {
        let handle = Arc::new(std::sync::RwLock::new(Some(Arc::new(ctx))));
        self.ty_ctx_handle = Some(handle);
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

    pub fn with_opt_level(mut self, level: u8) -> Self {
        self.opt_level = level;
        self
    }

    pub fn with_opt_for_size(mut self, size: bool) -> Self {
        self.opt_for_size = size;
        self
    }

    /// Request a link-time optimization strategy for this compilation
    /// (Phase 10.2). `LtoKind::None` is the default; `Fat` merges modules
    /// inside the compiler; `Thin` is a tracked gap (linker-driver integration)
    /// and will surface an error at codegen time rather than silently no-op.
    pub fn with_lto(mut self, lto: crate::passes::LtoKind) -> Self {
        self.lto = lto;
        self
    }

    /// Generate LLVM IR for a single MIR body without needing to set the TyCtx in the backend.
    pub fn emit_ir_to_string(&self, ctx: &TyCtx, body: &Body) -> CompResult<String> {
        let context = Context::create();
        let module = self.lower_body_to_module_with_ctx(&context, body, ctx)?;
        Ok(module.print_to_string().to_string())
    }

    pub fn emit_ir_to_string_with_handle(&self, body: &Body) -> CompResult<String> {
        let context = Context::create();
        let ty_ctx = self
            .ty_ctx_handle
            .as_ref()
            .and_then(|h| h.read().unwrap().clone())
            .ok_or_else(|| vec![GlyimDiagnostic::internal_error("no TyCtx available")])?;
        let module = self.lower_body_to_module_with_ctx(&context, body, ty_ctx.as_ref())?;
        Ok(module.print_to_string().to_string())
    }

    /// Lower the given MIR bodies into a single LLVM module and emit **assembly**
    /// (`.s`) to `output`, honoring the configured target triple and opt level.
    /// Mirrors [`CodegenBackend::generate`] but uses `FileType::Assembly` instead
    /// of `FileType::Object` (plan §18.3: `--emit=asm`).
    pub fn emit_assembly(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<()> {
        let context = Context::create();
        let module = context.create_module("glyim_module");
        let triple = TargetTriple::create(&self.target_triple);
        module.set_triple(&triple);
        let ty_ctx = self
            .ty_ctx_handle
            .as_ref()
            .and_then(|h| h.read().unwrap().clone())
            .ok_or_else(|| vec![GlyimDiagnostic::internal_error("no TyCtx available")])?;
        let ty_ctx = ty_ctx.as_ref();
        for body in bodies.iter() {
            crate::lower::lower_body(
                &context,
                &module,
                body,
                self.target_info.clone(),
                ty_ctx,
                self.debug_info,
                self.source_map.clone(),
                self.hygiene_ctx.clone(),
            )?;
        }
        let target = Target::from_triple(&triple).map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!("Target error: {}", e))]
        })?;
        let opt_level = match self.opt_level {
            0 => inkwell::OptimizationLevel::None,
            1 => inkwell::OptimizationLevel::Less,
            2 => inkwell::OptimizationLevel::Default,
            _ => inkwell::OptimizationLevel::Aggressive,
        };
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                opt_level,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .ok_or_else(|| {
                vec![GlyimDiagnostic::internal_error("Failed to create target machine")]
            })?;
        self.run_passes_on_module(&module, &target_machine)
            .map_err(|e| vec![GlyimDiagnostic::internal_error(e)])?;
        target_machine
            .write_to_file(&module, inkwell::targets::FileType::Assembly, output)
            .map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "Failed to write assembly file: {:?}",
                    e
                ))]
            })?;
        Ok(())
    }

    /// Lower a single body to an LLVM module using the provided TyCtx.
    pub fn lower_body_to_module_with_ctx<'ctx>(
        &self,
        context: &'ctx Context,
        body: &Body,
        ctx: &TyCtx,
    ) -> CompResult<inkwell::module::Module<'ctx>> {
        let module = context.create_module("glyim_module");
        let triple = TargetTriple::create(&self.target_triple);
        module.set_triple(&triple);
        crate::lower::lower_body(
            context,
            &module,
            body,
            self.target_info.clone(),
            ctx,
            self.debug_info,
            self.source_map.clone(),
            self.hygiene_ctx.clone(),
        )?;
        Ok(module)
    }

    pub(crate) fn run_passes_on_module<'ctx>(
        &self,
        module: &inkwell::module::Module<'ctx>,
        target_machine: &inkwell::targets::TargetMachine,
    ) -> Result<(), String> {
        // Honour the requested link-time optimization strategy (Phase 10.2).
        // `run_lto` treats `None`/`Fat` as running the standard pipeline once
        // (Fat with no secondary modules is just a single-module pass run), and
        // surfaces `Thin` as an explicit tracked-gap error rather than silently
        // no-op. Full multi-module merge requires the multi-CGU compilation
        // driver, which is a tracked gap (KNOWN_GAPS.md Phase 10.2).
        crate::passes::run_lto(
            module,
            &[],
            self.lto,
            target_machine,
            self.opt_level,
            self.opt_for_size,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn lower_body_to_module<'ctx>(
        &self,
        context: &'ctx Context,
        body: &Body,
    ) -> CompResult<inkwell::module::Module<'ctx>> {
        let module = context.create_module("test_module");
        let triple = inkwell::targets::TargetTriple::create(&self.target_triple);
        module.set_triple(&triple);
        let ty_ctx = self
            .ty_ctx_handle
            .as_ref()
            .and_then(|h| h.read().unwrap().clone())
            .ok_or_else(|| vec![GlyimDiagnostic::internal_error("no TyCtx available")])?;
        let ty_ctx = ty_ctx.as_ref();
        crate::lower::lower_body(
            context,
            &module,
            body,
            self.target_info.clone(),
            ty_ctx,
            self.debug_info,
            self.source_map.clone(),
            self.hygiene_ctx.clone(),
        )?;
        Ok(module)
    }

    #[allow(dead_code)]
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
            .ty_ctx_handle
            .as_ref()
            .and_then(|h| h.read().unwrap().clone())
            .ok_or_else(|| vec![GlyimDiagnostic::internal_error("no TyCtx available")])?;
        let ty_ctx = ty_ctx.as_ref();
        for body in bodies.iter() {
            crate::lower::lower_body(
                context,
                &module,
                body,
                self.target_info.clone(),
                ty_ctx,
                self.debug_info,
                self.source_map.clone(),
                self.hygiene_ctx.clone(),
            )?;
        }
        let target = Target::from_triple(&triple).map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!(
                "Target error: {}",
                e
            ))]
        })?;
        let opt_level = match self.opt_level {
            0 => inkwell::OptimizationLevel::None,
            1 => inkwell::OptimizationLevel::Less,
            2 => inkwell::OptimizationLevel::Default,
            _ => inkwell::OptimizationLevel::Aggressive,
        };
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                opt_level,
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
            .ty_ctx_handle
            .as_ref()
            .and_then(|h| h.read().unwrap().clone())
            .ok_or_else(|| vec![GlyimDiagnostic::internal_error("no TyCtx available")])?;
        let ty_ctx = ty_ctx.as_ref();
        crate::lower::lower_body(
            context,
            &module,
            body,
            self.target_info.clone(),
            ty_ctx,
            self.debug_info,
            self.source_map.clone(),
            self.hygiene_ctx.clone(),
        )?;
        let target = Target::from_triple(&triple).map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!(
                "Target error: {}",
                e
            ))]
        })?;
        let opt_level = match self.opt_level {
            0 => inkwell::OptimizationLevel::None,
            1 => inkwell::OptimizationLevel::Less,
            2 => inkwell::OptimizationLevel::Default,
            _ => inkwell::OptimizationLevel::Aggressive,
        };
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                opt_level,
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
