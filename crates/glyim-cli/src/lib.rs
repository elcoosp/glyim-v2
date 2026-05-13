use clap::Parser;
#[derive(Parser)]
#[command(name = "glyim", version, about = "The Glyim compiler")]
pub struct CliArgs {
    #[arg(value_name = "INPUT")]
    pub input: std::path::PathBuf,
}
pub fn run() -> Result<(), Vec<glyim_diag::GlyimDiagnostic>> { Ok(()) }
