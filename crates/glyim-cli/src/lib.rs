use clap::Parser;
use glyim_codegen::BytecodeBackend;
use glyim_codegen_llvm::LlvmBackend;
use glyim_db::{CrateConfig, Database};
use glyim_pipeline::Pipeline;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "glyim", version, about = "The Glyim compiler")]
pub struct CliArgs {
    #[arg(value_name = "INPUT")]
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long, value_name = "EMIT", default_value = "obj")]
    pub emit: String,
    #[arg(long, value_name = "EMIT", default_value = "obj")]
    #[arg(short = 'O', long = "opt-level", default_value = "0")]
    pub opt_level: u8,
    #[arg(long = "target")]
    pub target: Option<String>,
    #[arg(long = "backend", default_value = "llvm")]
    pub backend: String,
}

pub fn run() -> Result<(), Vec<glyim_diag::GlyimDiagnostic>> {
    let args = CliArgs::parse();
    run_with_args(args)
}

pub(crate) fn run_with_args(args: CliArgs) -> Result<(), Vec<glyim_diag::GlyimDiagnostic>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .ok();

    let output_path = args.output.unwrap_or_else(|| {
        let mut out = args.input.clone();
        out.set_extension("o");
        out
    });

    let config = CrateConfig {
        name: args
            .input
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        target_triple: args
            .target
            .clone()
            .unwrap_or_else(|| "x86_64-unknown-linux-gnu".to_string()),
        opt_level: args.opt_level,
    };

    let mut db = Database::new(config);

    let backend: Box<dyn glyim_codegen::CodegenBackend> = if args.backend == "bytecode" {
        Box::new(BytecodeBackend::new())
    } else {
        Box::new(LlvmBackend::with_target(
            args.target
                .unwrap_or_else(|| "x86_64-unknown-linux-gnu".to_string()),
        ))
    };

    Pipeline::compile_file(&mut db, &args.input, &*backend, &output_path)?;
    Ok(())
}

#[cfg(test)]
mod tests;
