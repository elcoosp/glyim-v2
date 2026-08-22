//! Crate root.
use clap::Parser;
use glyim_codegen::BytecodeBackend;
use glyim_codegen_llvm::LlvmBackend;
use glyim_codegen_llvm::passes::LtoKind;
use glyim_db::{CrateConfig, Database};
use glyim_pipeline::Pipeline;
use std::path::PathBuf;

/// linker.
pub mod linker;

#[derive(Parser, Debug)]
#[command(name = "glyim", version, about = "The Glyim compiler")]
/// CliArgs.
pub struct CliArgs {
    #[arg(value_name = "INPUT")]
/// Struct.
    pub input: PathBuf,
    #[arg(short, long)]
/// Struct.
    pub output: Option<PathBuf>,
    #[arg(long, value_name = "EMIT", default_value = "obj")]
/// Struct.
    pub emit: String,
    #[arg(short = 'O', long = "opt-level", default_value = "0")]
/// Struct.
    pub opt_level: u8,
    #[arg(long = "target")]
/// Struct.
    pub target: Option<String>,
    #[arg(long = "backend", default_value = "llvm")]
/// Struct.
    pub backend: String,
    #[arg(long = "linker")]
/// Struct.
    pub linker: Option<String>,
    #[arg(long = "link-flags")]
/// Struct.
    pub link_flags: Option<String>,
    /// Link-time optimization strategy: `off` (default), `fat` (in-compiler
    /// module merge + optimize), or `thin` (tracked gap — requires linker
    /// driver integration; surfaces an explicit error rather than silently
    /// no-op). Phase 10.2.
    #[arg(long = "lto", default_value = "off")]
    pub lto: String,
    /// Number of codegen units (CGUs) to partition monomorphized items into
    /// for parallel code generation. Defaults to the available parallelism
    /// (capped at 16), mirroring rustc's `-C codegen-units` policy. (Phase
    /// 10.2 / feature-gaps §4.1.)
    #[arg(long = "codegen-units")]
    pub codegen_units: Option<usize>,
}

/// run.
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
    Asm,
    /// Emit a position-independent shared library (cdylib). Phase 9.2: this is
    /// the host artifact a procedural-macro crate compiles to so it can be
    /// `dlopen`ed by `glyim_proc_macro::load_cdylib` during macro expansion.
    Cdylib,
}

impl EmitKind {
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "obj" => Ok(EmitKind::Obj),
            "exec" => Ok(EmitKind::Exec),
            "mir" => Ok(EmitKind::Mir),
            "llvm-ir" => Ok(EmitKind::LlvmIr),
            "asm" => Ok(EmitKind::Asm),
            "cdylib" => Ok(EmitKind::Cdylib),
            _ => Err(format!(
                "invalid value for --emit: '{}' (expected one of: obj, exec, mir, llvm-ir, asm, cdylib)",
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
        EmitKind::Cdylib => {
            // Compile to a private object, then link it into a shared library.
            let obj = {
                let mut p = input.clone();
                p.set_extension("o");
                p
            };
            let final_out = args.output.clone().unwrap_or_else(|| {
                let mut p = input.clone();
                p.set_extension(if cfg!(target_os = "macos") {
                    "dylib"
                } else {
                    "so"
                });
                p
            });
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
        EmitKind::Asm => {
            let out = args.output.clone().unwrap_or_else(|| {
                let mut p = input.clone();
                p.set_extension("s");
                p
            });
            (out, None)
        }
    };

    let target_triple = args
        .target
        .clone()
        .unwrap_or_else(|| "x86_64-unknown-linux-gnu".to_string());

    // Parse the requested LTO strategy (Phase 10.2). `Thin` is a tracked gap
    // (linker-driver integration); surface it as an explicit error rather than
    // silently degrading to a no-op.
    let lto = match args.lto.as_str() {
        "off" | "none" | "false" => LtoKind::None,
        "fat" => LtoKind::Fat,
        "thin" => LtoKind::Thin,
        other => {
            return Err(vec![glyim_diag::GlyimDiagnostic::parse_error(
                glyim_diag::Span::DUMMY,
                format!(
                    "invalid value for --lto: '{}' (expected one of: off, fat, thin)",
                    other
                ),
            )]);
        }
    };

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

    // Early return for MIR, LLVM IR, and assembly emit
    if emit == EmitKind::Mir {
        return glyim_pipeline::emit_mir(&mut db, input, &object_path);
    } else if emit == EmitKind::LlvmIr {
        return glyim_pipeline::emit_llvm_ir(&mut db, input, &object_path);
    } else if emit == EmitKind::Asm {
        return glyim_pipeline::emit_asm(&mut db, input, &object_path);
    }

    // Build the LLVM backend concretely so the Thin path can drive per-CGU
    // bitcode emission (`emit_thinlto_bitcode_files`) directly. For the
    // non-Thin paths it is boxed into the `CodegenBackend` trait object.
    let entry_main = glyim_pipeline::Pipeline::entry_main_local_id(&mut db, input);
    let target_info = glyim_core::TargetInfo::from_triple(&target_triple);
    let mut llvm = LlvmBackend::with_db(&db)
        .with_target(&target_triple)
        .with_opt_level(args.opt_level)
        .with_opt_for_size(false)
        .with_lto(lto);
    // Only emit a C-ABI `main` entry symbol for `--emit=exec`; a cdylib or
    // plain object must not carry a `main` (it would be an unused/conflicting
    // entry point). `obj`/`cdylib` consumers link `main` themselves if needed.
    if let (Some(main_id), EmitKind::Exec) = (entry_main, emit) {
        llvm = llvm.with_entry_main(main_id);
    }

    // ThinLTO: emit one bitcode file per codegen unit, then run the thin-link
    // driver (`thin_lto_link`, which shells out to `llvm-lto2`) to combine them
    // incrementally. The thin-linked object is written to `object_path` for the
    // final linker step below. `compile_file_with_artifacts` runs the full
    // pipeline; its internal `generate` call short-circuits for Thin (see
    // `LlvmBackend::generate`) so it does not write a redundant merged object.
    if lto == LtoKind::Thin {
        if args.backend == "bytecode" {
            return Err(vec![glyim_diag::GlyimDiagnostic::internal_error(
                "ThinLTO requires the LLVM backend; `--backend=bytecode` cannot emit \
                 per-CGU bitcode. Use the default (LLVM) backend for `--lto=thin`.",
            )]);
        }
        let artifacts = glyim_pipeline::Pipeline::compile_file_with_artifacts(
            &mut db,
            input,
            &llvm,
            &object_path,
            args.codegen_units,
        )?;
        let bitcode_dir = object_path.with_extension("thin-bc");
        std::fs::create_dir_all(&bitcode_dir).map_err(|e| {
            vec![glyim_diag::GlyimDiagnostic::internal_error(&format!(
                "ThinLTO: failed to create bitcode dir {}: {}",
                bitcode_dir.display(),
                e
            ))]
        })?;
        let bitcode_paths = llvm
            .emit_thinlto_bitcode_files(&artifacts.mir_bodies, &bitcode_dir)
            .map_err(|e| {
                vec![glyim_diag::GlyimDiagnostic::internal_error(&format!(
                    "ThinLTO per-CGU bitcode emission failed: {:?}",
                    e
                ))]
            })?;
        let thin_objects =
            linker::thin_lto_link(&bitcode_paths, args.opt_level, &bitcode_dir).map_err(|e| {
                vec![glyim_diag::GlyimDiagnostic::internal_error(&format!(
                    "ThinLTO thin-link failed: {}",
                    e
                ))]
            })?;
        let thin_obj = thin_objects.into_iter().next().ok_or_else(|| {
            vec![glyim_diag::GlyimDiagnostic::internal_error(
                "ThinLTO thin-link produced no object files",
            )]
        })?;
        std::fs::copy(&thin_obj, &object_path).map_err(|e| {
            vec![glyim_diag::GlyimDiagnostic::internal_error(&format!(
                "ThinLTO: failed to copy thin-linked object to {}: {}",
                object_path.display(),
                e
            ))]
        })?;
    } else if args.backend == "bytecode" {
        if args.opt_level > 0 {
            tracing::warn!(
                "bytecode backend opt-level currently has no effect; reserved for future peephole passes"
            );
        }
        let ctx = glyim_type::TyCtxMut::new(db.interner().clone()).freeze();
        let backend: Box<dyn glyim_codegen::CodegenBackend> =
            Box::new(BytecodeBackend::with_ty_ctx(std::sync::Arc::new(ctx), target_info));
        Pipeline::compile_file(&mut db, input, &*backend, &object_path, args.codegen_units)?;
    } else {
        let backend: Box<dyn glyim_codegen::CodegenBackend> = Box::new(llvm);
        Pipeline::compile_file(&mut db, input, &*backend, &object_path, args.codegen_units)?;
    }

    if emit == EmitKind::Exec || emit == EmitKind::Cdylib {
        let final_path = final_output_path.expect("emit should have final output");
        // `cdylib` produces a position-independent shared library (`-shared`);
        // `exec` produces a runnable binary. Phase 9.2: the cdylib is the host
        // artifact a proc-macro crate compiles to so `load_cdylib` can dlopen it.
        let extra_flags = if emit == EmitKind::Cdylib {
            Some("-shared")
        } else {
            None
        };
        linker::invoke_linker(
            &object_path,
            &final_path,
            args.linker.as_deref(),
            extra_flags.or(args.link_flags.as_deref()),
            args.target.as_deref(),
        )
        .map_err(|e| vec![glyim_diag::GlyimDiagnostic::internal_error(&e)])?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;
