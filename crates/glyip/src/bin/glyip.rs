//! Binary entry point for the `glyip` build tool.

use clap::Parser;

/// Cargo-like build tool for the Glyim compiler.
#[derive(Parser, Debug)]
#[command(name = "glyip", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Create a new Glyim project.
    New {
        /// Project name.
        name: String,
        /// Create a library project.
        #[arg(long)]
        lib: bool,
        /// Language edition.
        #[arg(long, default_value = "2024")]
        edition: String,
    },
    /// Compile the project.
    Build {
        /// Build in release mode.
        #[arg(long)]
        release: bool,
        /// Target triple.
        #[arg(long)]
        target: Option<String>,
        /// Codegen backend.
        #[arg(long, default_value = "bytecode")]
        backend: String,
        /// Optimisation level (0–3).
        #[arg(long, default_value_t = 0)]
        opt_level: u8,
    },
    /// Run the project's tests.
    Test {
        /// Build in release mode.
        #[arg(long)]
        release: bool,
        /// Only run tests matching this substring.
        #[arg(long)]
        filter: Option<String>,
        /// Compile tests but don't run them.
        #[arg(long)]
        no_run: bool,
    },
    /// Build and execute the project's binary.
    Run {
        /// Build in release mode.
        #[arg(long)]
        release: bool,
        /// Arguments to pass to the binary.
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

fn main() {
    // Initialise the tracing subscriber (minimal, to stderr).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let cli = Cli::parse();
    let project_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let result = match cli.command {
        Commands::New { name, lib, edition } => {
            let opts = glyip::config::NewOptions { lib, edition };
            glyip::cmd_new(&name, &opts).map(|_| ())
        }
        Commands::Build {
            release,
            target,
            backend,
            opt_level,
        } => {
            let opts = glyip::config::BuildOptions {
                release,
                target,
                backend,
                opt_level,
            };
            glyip::cmd_build(&project_dir, &opts).map(|_| ())
        }
        Commands::Test {
            release,
            filter,
            no_run,
        } => {
            let opts = glyip::config::TestOptions {
                release,
                filter,
                no_run,
            };
            glyip::cmd_test(&project_dir, &opts).map(|_| ())
        }
        Commands::Run { release, args } => {
            let opts = glyip::config::RunOptions {
                release,
                args,
                backend: "bytecode".to_string(),
            };
            glyip::cmd_run(&project_dir, &opts).map(|_| ())
        }
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
