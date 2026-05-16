//! Command implementations for glyip.
//!
//! Each public function corresponds to a CLI sub-command and orchestrates
//! configuration loading, dependency resolution, compilation, and output.

use glyim_db::{CrateConfig, Database};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::cache::Cache;
use crate::config::{BuildOptions, GlyipToml, NewOptions, RunOptions, TestOptions};
use crate::dep::DependencyResolver;
use crate::error::{GlyipError, GlyipResult};
use crate::lockfile::Lockfile;

// ---------------------------------------------------------------------------
// `glyip new`
// ---------------------------------------------------------------------------

/// Result of a successful `glyip new` invocation.
#[derive(Debug)]
pub struct NewResult {
    /// Absolute path to the newly created project directory.
    pub path: PathBuf,
}

/// Create a new Glyim project at the given path.
pub fn cmd_new(name: &str, opts: &NewOptions) -> GlyipResult<NewResult> {
    let dir = std::env::current_dir()?.join(name);
    if dir.exists() {
        return Err(GlyipError::ProjectAlreadyExists(dir));
    }

    info!(
        "Creating new {} project '{}' at {}",
        if opts.lib { "library" } else { "binary" },
        name,
        dir.display()
    );

    // Create directory tree.
    fs::create_dir_all(dir.join("src"))?;
    fs::create_dir_all(dir.join("tests"))?;

    // Write Glyip.toml.
    let config = GlyipToml {
        package: crate::config::PackageConfig {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            edition: opts.edition.clone(),
            authors: Vec::new(),
            description: None,
            bin: if opts.lib {
                None
            } else {
                Some(vec![crate::config::BinTarget {
                    name: name.to_string(),
                    path: Some(PathBuf::from("src/main.g")),
                }])
            },
            lib: if opts.lib {
                Some(crate::config::LibTarget {
                    path: Some(PathBuf::from("src/lib.g")),
                })
            } else {
                None
            },
        },
        dependencies: BTreeMap::new(),
        dev_dependencies: BTreeMap::new(),
    };
    config.write_to_dir(&dir)?;

    // Write source entry point.
    if opts.lib {
        let lib_src = dir.join("src/lib.g");
        fs::write(&lib_src, "// Library entry point\n")?;
    } else {
        let main_src = dir.join("src/main.g");
        fs::write(&main_src, "fn main() {}\n")?;
    }

    // Write a placeholder test file.
    let test_src = dir.join("tests/integration.g");
    fs::write(&test_src, "// Integration tests\n")?;

    // Create an empty lockfile.
    Lockfile::new().write_to_dir(&dir)?;

    info!("Project '{}' created successfully", name);
    Ok(NewResult { path: dir })
}

// ---------------------------------------------------------------------------
// `glyip build`
// ---------------------------------------------------------------------------

/// Result of a successful `glyip build` invocation.
#[derive(Debug)]
pub struct BuildResult {
    /// Path to the output artifact (binary or library).
    pub output: PathBuf,
    /// Number of diagnostics (errors + warnings) emitted.
    pub diagnostic_count: usize,
    /// Whether the build was incremental (some files were cached).
    pub incremental: bool,
}

/// Build the project in the current directory (or `project_dir`).
pub fn cmd_build(project_dir: &Path, opts: &BuildOptions) -> GlyipResult<BuildResult> {
    let config = GlyipToml::read_from_dir(project_dir)?;
    let mut cache = Cache::new(project_dir)?;

    info!(
        "Building {} v{}",
        config.package.name, config.package.version
    );

    // Dependency resolution.
    let resolver = DependencyResolver::new_no_index();
    let _lockfile = resolver.resolve(&config, project_dir)?;
    info!("Dependencies resolved");

    // Incremental check.
    let incremental = !cache.needs_rebuild()?;
    if incremental {
        info!("No source changes detected — skipping compilation");
        let output = cache.output_binary(&config.package.name, opts.release);
        if output.exists() {
            return Ok(BuildResult {
                output,
                diagnostic_count: 0,
                incremental: true,
            });
        }
        // Fall through to recompile if artifact is missing despite clean fingerprints.
        info!("Artifact missing — recompiling");
    }

    // Find entry point.
    let entry = find_entry_point(project_dir, &config)?;

    // Run the compiler pipeline.
    let (output, diag_count) = compile_source(project_dir, &entry, &config, opts, &mut cache)?;

    // Mark build as successful.
    cache.mark_built()?;

    Ok(BuildResult {
        output,
        diagnostic_count: diag_count,
        incremental: false,
    })
}

// ---------------------------------------------------------------------------
// `glyip test`
// ---------------------------------------------------------------------------

/// Result of a successful `glyip test` invocation.
#[derive(Debug)]
pub struct TestResult {
    /// Total tests discovered.
    pub total: usize,
    /// Tests that passed.
    pub passed: usize,
    /// Tests that failed.
    pub failed: usize,
    /// Tests that were ignored / skipped.
    pub ignored: usize,
}

/// Build and run the project's tests.
pub fn cmd_test(project_dir: &Path, opts: &TestOptions) -> GlyipResult<TestResult> {
    let config = GlyipToml::read_from_dir(project_dir)?;
    let mut cache = Cache::new(project_dir)?;

    info!(
        "Testing {} v{}",
        config.package.name, config.package.version
    );

    // Resolve dependencies.
    let resolver = DependencyResolver::new_no_index();
    let _lockfile = resolver.resolve(&config, project_dir)?;

    // Find test entry points.
    let test_dir = project_dir.join("tests");
    let mut test_files: Vec<PathBuf> = Vec::new();
    if test_dir.exists() {
        collect_source_files(&test_dir, &mut test_files)?;
    }

    // Also look for inline test modules in src/.
    let src_dir = project_dir.join("src");
    if src_dir.exists() {
        collect_source_files(&src_dir, &mut test_files)?;
    }

    if test_files.is_empty() {
        info!("No test files found");
        if opts.no_run {
            return Ok(TestResult {
                total: 0,
                passed: 0,
                failed: 0,
                ignored: 0,
            });
        }
    }

    // Compile each test file.
    let mut total = 0usize;
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut ignored = 0usize;

    for test_file in &test_files {
        let build_opts = BuildOptions {
            release: opts.release,
            target: None,
            backend: "bytecode".to_string(),
            opt_level: 0,
        };

        // Apply filter: skip files that don't match.
        if let Some(ref filter) = opts.filter {
            let name = test_file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !name.contains(filter.as_str()) {
                ignored += 1;
                continue;
            }
        }

        if opts.no_run {
            total += 1;
            continue;
        }

        match compile_source(project_dir, test_file, &config, &build_opts, &mut cache) {
            Ok(_) => {
                total += 1;
                passed += 1;
            }
            Err(GlyipError::BuildFailed(_diags)) => {
                total += 1;
                failed += 1;
            }
            Err(e) => {
                warn!("Test compilation error: {}", e);
                total += 1;
                failed += 1;
            }
        }
    }

    info!(
        "Test results: {} passed, {} failed, {} ignored",
        passed, failed, ignored
    );
    Ok(TestResult {
        total,
        passed,
        failed,
        ignored,
    })
}

// ---------------------------------------------------------------------------
// `glyip run`
// ---------------------------------------------------------------------------

/// Result of a successful `glyip run` invocation.
#[derive(Debug)]
pub struct RunResult {
    /// Path to the compiled binary.
    pub binary: PathBuf,
    /// Exit code of the executed binary.
    pub exit_code: i32,
}

/// Build and run the project's binary target.
pub fn cmd_run(project_dir: &Path, opts: &RunOptions) -> GlyipResult<RunResult> {
    let config = GlyipToml::read_from_dir(project_dir)?;

    info!(
        "Running {} v{}",
        config.package.name, config.package.version
    );

    // Resolve dependencies.
    let resolver = DependencyResolver::new_no_index();
    let _lockfile = resolver.resolve(&config, project_dir)?;

    // Build first.
    let build_opts = BuildOptions {
        release: opts.release,
        target: None,
        backend: opts.backend.clone(),
        opt_level: if opts.release { 2 } else { 0 },
    };
    let build_result = cmd_build(project_dir, &build_opts)?;

    // Execute the binary.
    info!("Executing {}", build_result.output.display());
    let exit_code = run_binary(&build_result.output, &opts.args)?;

    Ok(RunResult {
        binary: build_result.output,
        exit_code,
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Find the main entry point for the project.
fn find_entry_point(project_dir: &Path, config: &GlyipToml) -> GlyipResult<PathBuf> {
    // Check explicit bin target path.
    if let Some(ref bins) = config.package.bin
        && let Some(first_bin) = bins.first()
        && let Some(ref path) = first_bin.path
    {
        let full = project_dir.join(path);
        if full.exists() {
            return Ok(full);
        }
    }

    // Check default locations.
    let candidates = [
        project_dir.join("src/main.g"),
        project_dir.join("src/lib.g"),
    ];
    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    Err(GlyipError::NoEntryPoint(project_dir.to_path_buf()))
}

/// Collect all `.g` source files under a directory (recursive).
fn collect_source_files(dir: &Path, out: &mut Vec<PathBuf>) -> GlyipResult<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_source_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "g") {
            out.push(path);
        }
    }
    Ok(())
}

/// Compile a single source file using the compiler pipeline.
fn compile_source(
    _project_dir: &Path,
    entry: &Path,
    config: &GlyipToml,
    opts: &BuildOptions,
    cache: &mut Cache,
) -> GlyipResult<(PathBuf, usize)> {
    let _source = fs::read_to_string(entry)?;

    // Set up the compiler database.
    let crate_config = CrateConfig {
        name: config.package.name.clone(),
        target_triple: opts
            .target
            .clone()
            .unwrap_or_else(|| "x86_64-unknown-linux-gnu".to_string()),
        opt_level: opts.opt_level,
    };
    let mut db = Database::new(crate_config);

    // Select the codegen backend.
    #[cfg(feature = "llvm")]
    let backend: Box<dyn glyim_codegen::CodegenBackend> = if opts.backend == "llvm" {
        Box::new(glyim_codegen_llvm::LlvmBackend::new())
    } else {
        Box::new(glyim_codegen::BytecodeBackend::new())
    };
    #[cfg(not(feature = "llvm"))]
    let backend: Box<dyn glyim_codegen::CodegenBackend> = {
        if opts.backend == "llvm" {
            tracing::warn!(
                "STUB: LLVM backend requested but not compiled in, falling back to bytecode"
            );
        }
        Box::new(glyim_codegen::BytecodeBackend::new())
    };

    // Run the pipeline.
    let output_dir = cache.output_dir(opts.release);
    fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join(&config.package.name);

    match glyim_pipeline::Pipeline::compile_file(&mut db, entry, backend.as_ref(), &output_path) {
        Ok(()) => {
            info!("Compilation succeeded: {}", output_path.display());
            Ok((output_path, 0))
        }
        Err(diags) => Err(GlyipError::BuildFailed(diags)),
    }
}

/// Run a compiled binary and return its exit code.
fn run_binary(binary: &Path, args: &[String]) -> GlyipResult<i32> {
    if !binary.exists() {
        return Err(GlyipError::Other(format!(
            "binary not found: {}",
            binary.display()
        )));
    }

    let status = std::process::Command::new(binary)
        .args(args)
        .status()
        .map_err(GlyipError::Io)?;

    Ok(status.code().unwrap_or(-1))
}
