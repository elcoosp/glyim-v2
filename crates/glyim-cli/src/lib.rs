use clap::Parser;
use glyim_db::{Database, CrateConfig};
use glyim_pipeline::Pipeline;
use glyim_codegen_llvm::LlvmBackend;
use glyim_codegen::CodegenBackend;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "glyim", version, about = "The Glyim compiler")]
pub struct CliArgs {
    #[arg(value_name = "INPUT")]
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(short = 'O', long = "opt-level", default_value = "0")]
    pub opt_level: u8,
    #[arg(long = "target")]
    pub target: Option<String>,
}

pub fn run() -> Result<(), Vec<glyim_diag::GlyimDiagnostic>> {
    let args = CliArgs::parse();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let output = args.output.unwrap_or_else(|| {
        let mut out = args.input.clone();
        out.set_extension("o");
        out
    });

    let config = CrateConfig {
        name: args.input.file_stem().unwrap_or_default().to_string_lossy().to_string(),
        target_triple: args.target.clone().unwrap_or_else(|| "x86_64-unknown-linux-gnu".to_string()),
        opt_level: args.opt_level,
    };

    let mut db = Database::new(config);
    let backend = LlvmBackend::with_target(args.target.unwrap_or_else(|| "x86_64-unknown-linux-gnu".to_string()));

    Pipeline::compile_file(&mut db, &args.input, &backend)?;
    Ok(())
}
