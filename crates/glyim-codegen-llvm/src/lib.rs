use glyim_codegen::CodegenBackend;
use glyim_diag::{CompResult, GlyimDiagnostic};
use glyim_mir::Body;
use inkwell::context::Context;
use inkwell::targets::{InitializationConfig, Target, TargetTriple};
use std::path::Path;
use std::sync::Arc;

pub struct LlvmBackend {
    context: Context,
    target_triple: String,
}

impl Default for LlvmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl LlvmBackend {
    pub fn new() -> Self {
        Target::initialize_all(&InitializationConfig::default());
        Self {
            context: Context::create(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
        }
    }

    pub fn with_target(target_triple: impl Into<String>) -> Self {
        Target::initialize_all(&InitializationConfig::default());
        Self {
            context: Context::create(),
            target_triple: target_triple.into(),
        }
    }
}

impl CodegenBackend for LlvmBackend {
    fn name(&self) -> &'static str {
        "llvm"
    }

    fn generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<Vec<u8>> {
        let module = self.context.create_module("glyim_module");
        let triple = TargetTriple::create(&self.target_triple);
        module.set_triple(&triple);

        for (idx, _body) in bodies.iter().enumerate() {
            let fn_name = format!("func_{}", idx);
            let i32_type = self.context.i32_type();
            let fn_type = i32_type.fn_type(&[], false);
            let function = module.add_function(&fn_name, fn_type, None);
            let basic_block = self.context.append_basic_block(function, "entry");
            let builder = self.context.create_builder();
            builder.position_at_end(basic_block);
            let return_val = i32_type.const_int(42, false);
            let _ = builder.build_return(Some(&return_val));
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

        target_machine
            .write_to_file(&module, inkwell::targets::FileType::Object, output)
            .map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "Failed to write object file: {:?}",
                    e
                ))]
            })?;

        Ok(Vec::new())
    }

    fn generate_function(&self, _body: &Arc<Body>) -> CompResult<Vec<u8>> {
        Ok(Vec::new())
    }
}
