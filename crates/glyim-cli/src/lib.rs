use clap::Parser;

#[derive(Parser)]
#[command(name = "glyim", version, about = "The Glyim compiler")]
pub struct CliArgs {
    #[arg(value_name = "INPUT")]
    pub input: std::path::PathBuf,
    #[arg(short, long)]
    pub output: Option<std::path::PathBuf>,
    #[arg(long = "emit-llvm")]
    pub emit_llvm: bool,
    #[arg(short, long)]
    pub verbose: bool,
    #[arg(short = 'O', long = "opt-level", default_value = "0")]
    pub opt_level: u8,
    #[arg(long = "target")]
    pub target: Option<String>,
    #[arg(long = "backend", default_value = "llvm")]
    pub backend: String,
}

pub fn run() -> Result<(), Vec<glyim_diag::GlyimDiagnostic>> {
    let args = CliArgs::parse();
    if args.verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }
    let config = glyim_db::CrateConfig {
        name: "main".to_string(),
        target_triple: args.target.clone().unwrap_or_else(|| "x86_64-unknown-linux-gnu".to_string()),
        opt_level: args.opt_level,
    };
    let mut db = glyim_db::Database::new(config);
    let backend = glyim_codegen_llvm::LlvmBackend::new();
    glyim_pipeline::Pipeline::compile_file(&mut db, &args.input, &backend)?;
    Ok(())
}
