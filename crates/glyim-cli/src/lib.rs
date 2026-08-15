#![allow(missing_docs)]
use clap::Parser;
use glyim_codegen::BytecodeBackend;
use glyim_codegen_llvm::LlvmBackend;
use glyim_db::{CrateConfig, Database};
use glyim_pipeline::Pipeline;
use std::path::PathBuf;

pub mod linker;

#[derive(Parser, Debug)]
#[command(name = "glyim", version, about = "The Glyim compiler")]
pub struct CliArgs {
    #[arg(value_name = "INPUT")]
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long, value_name = "EMIT", default_value = "obj")]
    pub emit: String,
    #[arg(short = 'O', long = "opt-level", default_value = "0")]
    pub opt_level: u8,
    #[arg(long = "target")]
    pub target: Option<String>,
    #[arg(long = "backend", default_value = "llvm")]
    pub backend: String,
    #[arg(long = "linker")]
    pub linker: Option<String>,
    #[arg(long = "link-flags")]
    pub link_flags: Option<String>,
}

pub fn run() -> Result<(), Vec<glyim_diag::GlyimDiagnostic>> {
    let args = CliArgs::parse();
    run_with_args(args)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EmitKind {
    Obj,
    Exec,
    Mir,
    LlvmIr,
}

impl EmitKind {
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "obj" => Ok(EmitKind::Obj),
            "exec" => Ok(EmitKind::Exec),
            "mir" => Ok(EmitKind::Mir),
            "llvm-ir" => Ok(EmitKind::LlvmIr),
            _ => Err(format!(
                "invalid value for --emit: '{}' (expected one of: obj, exec, mir, llvm-ir)",
                s
            )),
        }
    }
}

pub(crate) fn run_with_args(args: CliArgs) -> Result<(), Vec<glyim_diag::GlyimDiagnostic>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .ok();

    let input = &args.input;
    let emit = match EmitKind::from_str(&args.emit) {
        Ok(k) => k,
        Err(msg) => {
            return Err(vec![glyim_diag::GlyimDiagnostic::parse_error(
                glyim_diag::Span::DUMMY,
                msg,
            )]);
        }
    };

    // Determine output paths based on emit mode
    let (object_path, final_output_path) = match emit {
        EmitKind::Obj | EmitKind::Exec => {
            let obj = args.output.clone().unwrap_or_else(|| {
                let mut p = input.clone();
                p.set_extension("o");
                p
            });
            let final_out = if emit == EmitKind::Exec {
                args.output.clone().unwrap_or_else(|| {
                    let mut p = input.clone();
                    p.set_extension("");
                    p
                })
            } else {
                obj.clone()
            };
            (obj, Some(final_out))
        }
        EmitKind::Mir | EmitKind::LlvmIr => {
            let out = args.output.clone().unwrap_or_else(|| {
                let mut p = input.clone();
                let ext = if emit == EmitKind::Mir { "mir" } else { "ll" };
                p.set_extension(ext);
                p
            });
            (out, None)
        }
    };

    let target_triple = args
        .target
        .clone()
        .unwrap_or_else(|| "x86_64-unknown-linux-gnu".to_string());

    let config = CrateConfig {
        name: args
            .input
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        target_triple: target_triple.clone(),
        opt_level: args.opt_level,
    };

    let mut db = Database::new(config);

    // Early return for MIR and LLVM IR emit
    if emit == EmitKind::Mir {
        return glyim_pipeline::emit_mir(&mut db, input, &object_path);
    } else if emit == EmitKind::LlvmIr {
        return glyim_pipeline::emit_llvm_ir(&mut db, input, &object_path);
    }

    // For obj and exec, compile to object
    let target_info = glyim_core::TargetInfo::from_triple(&target_triple);
    let backend: Box<dyn glyim_codegen::CodegenBackend> = if args.backend == "bytecode" {
        if args.opt_level > 0 {
            tracing::warn!(
                "bytecode backend opt-level currently has no effect; reserved for future peephole passes"
            );
        }
        let ctx = glyim_type::TyCtxMut::new(db.interner().clone()).freeze();
        Box::new(BytecodeBackend::with_ty_ctx(
            std::sync::Arc::new(ctx),
            target_info,
        ))
    } else {
        Box::new(
            LlvmBackend::with_db(&db)
                .with_target(&target_triple)
                .with_opt_level(args.opt_level)
                .with_opt_for_size(false),
        )
    };

    Pipeline::compile_file(&mut db, input, &*backend, &object_path)?;

    if emit == EmitKind::Exec {
        let final_path = final_output_path.expect("exec should have final output");
        linker::invoke_linker(
            &object_path,
            &final_path,
            args.linker.as_deref(),
            args.link_flags.as_deref(),
        )
        .map_err(|e| vec![glyim_diag::GlyimDiagnostic::internal_error(&e)])?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;
