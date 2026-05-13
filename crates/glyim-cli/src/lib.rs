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
    Ok(())
}
