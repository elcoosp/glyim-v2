use glyim_codegen::CodegenBackend;
use glyim_diag::{CompResult, GlyimDiagnostic};
use glyim_mir::{BasicBlockIdx, Body, Terminator, TerminatorKind};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{InitializationConfig, Target, TargetTriple};
use std::collections::HashMap;
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

    fn generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<()> {
        let context = &self.context;
        let module = context.create_module("glyim_module");
        let triple = TargetTriple::create(&self.target_triple);
        module.set_triple(&triple);

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

        for body in bodies.iter() {
            self.lower_body(context, &module, body)?;
        }

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

        self.lower_body(context, &module, body)?;

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

impl LlvmBackend {
    fn lower_body<'ctx>(
        &'ctx self,
        context: &'ctx Context,
        module: &Module<'ctx>,
        body: &Body,
    ) -> CompResult<()> {
        let fn_name = format!(
            "func_{}_{}",
            body.owner.krate.to_raw(),
            body.owner.local_id.to_raw()
        );

        let void_type = context.void_type();
        let fn_type = void_type.fn_type(&[], false);
        let function = module.add_function(&fn_name, fn_type, None);
        let entry_block = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry_block);

        let mut bb_map: HashMap<BasicBlockIdx, inkwell::basic_block::BasicBlock<'ctx>> =
            HashMap::new();
        bb_map.insert(BasicBlockIdx::from_raw(0), entry_block);

        for (bb_idx, _bb_data) in body.basic_blocks.iter_enumerated() {
            if bb_idx != BasicBlockIdx::from_raw(0) {
                let bb_name = format!("bb_{}", bb_idx.index());
                let llvm_bb = context.append_basic_block(function, &bb_name);
                bb_map.insert(bb_idx, llvm_bb);
            }
        }

        for (bb_idx, bb_data) in body.basic_blocks.iter_enumerated() {
            let llvm_bb = bb_map.get(&bb_idx).unwrap();
            builder.position_at_end(*llvm_bb);
            self.lower_terminator(context, &builder, &bb_data.terminator, &bb_map)?;
        }

        Ok(())
    }

    fn lower_terminator<'ctx>(
        &self,
        context: &Context,
        builder: &Builder<'ctx>,
        terminator: &Terminator,
        bb_map: &HashMap<BasicBlockIdx, inkwell::basic_block::BasicBlock<'ctx>>,
    ) -> CompResult<()> {
        match &terminator.kind {
            TerminatorKind::Goto { target } => {
                let target_bb = bb_map.get(target).unwrap();
                builder
                    .build_unconditional_branch(*target_bb)
                    .map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "Failed to build unconditional branch: {:?}",
                            e
                        ))]
                    })?;
            }
            TerminatorKind::Return => {
                builder.build_return(None).map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "Failed to build return: {:?}",
                        e
                    ))]
                })?;
            }
            TerminatorKind::Unreachable => {
                builder.build_unreachable().map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "Failed to build unreachable: {:?}",
                        e
                    ))]
                })?;
            }
            TerminatorKind::SwitchInt {
                discr: _,
                switch_ty: _,
                targets,
            } => {
                let otherwise_bb = bb_map.get(&targets.otherwise()).unwrap();
                let i32_type = context.i32_type();
                let default_case = i32_type.const_int(0, false);

                let mut cases = Vec::new();
                for (value, target_bb) in targets.iter() {
                    let target_block = bb_map.get(&target_bb).unwrap();
                    cases.push((i32_type.const_int(value as u64, false), *target_block));
                }

                builder
                    .build_switch(default_case, *otherwise_bb, &cases)
                    .map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "Failed to build switch: {:?}",
                            e
                        ))]
                    })?;
            }
            _ => {
                eprintln!("STUB: Terminator kind not yet implemented in LLVM lowering");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
