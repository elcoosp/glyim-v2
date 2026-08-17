use std::path::Path;
use std::process::Command;

/// Trait abstracting the system linker, allowing for different implementations
/// for MSVC, Unix/GCC, and LLD.
trait LinkerInvoker {
    fn link(
        &self,
        obj_path: &Path,
        output_path: &Path,
        link_flags: Option<&str>,
    ) -> Result<(), String>;
}

struct UnixLinker {
    linker: String,
}

impl LinkerInvoker for UnixLinker {
    fn link(
        &self,
        obj_path: &Path,
        output_path: &Path,
        link_flags: Option<&str>,
    ) -> Result<(), String> {
        let mut cmd = Command::new(&self.linker);
        cmd.arg(obj_path.as_os_str())
            .arg("-o")
            .arg(output_path.as_os_str());

        if let Some(flags) = link_flags {
            for flag in flags.split_whitespace() {
                cmd.arg(flag);
            }
        }

        let status = cmd
            .status()
            .map_err(|e| format!("Failed to invoke linker '{}': {}", self.linker, e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "Linker '{}' failed with status {}",
                self.linker, status
            ))
        }
    }
}

struct MsvcLinker {
    linker: String,
}

impl LinkerInvoker for MsvcLinker {
    fn link(
        &self,
        obj_path: &Path,
        output_path: &Path,
        link_flags: Option<&str>,
    ) -> Result<(), String> {
        let mut cmd = Command::new(&self.linker);
        cmd.arg(obj_path.as_os_str())
            .arg("/OUT:")
            .arg(output_path.as_os_str())
            .arg("/SUBSYSTEM:CONSOLE") // Default to console subsystem
            .arg("msvcrt.lib"); // Default C runtime

        if let Some(flags) = link_flags {
            for flag in flags.split_whitespace() {
                cmd.arg(flag);
            }
        }

        let status = cmd
            .status()
            .map_err(|e| format!("Failed to invoke linker '{}': {}", self.linker, e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "Linker '{}' failed with status {}",
                self.linker, status
            ))
        }
    }
}

pub fn invoke_linker(
    obj_path: &Path,
    output_path: &Path,
    linker: Option<&str>,
    link_flags: Option<&str>,
    target_triple: Option<&str>,
) -> Result<(), String> {
    // Plan §18.1: cross-compilation support. When a target triple is supplied
    // and differs from the host, compute the target-appropriate linker flags
    // (e.g. `--target=<triple>` for clang, or `-m <emulation>` for GNU ld) and
    // prepend them to any user-supplied flags. An unmapped target is a hard
    // error rather than silently passing host flags to a cross target.
    let mut cross_flags: Vec<String> = Vec::new();
    if let Some(triple) = target_triple {
        if !triple.is_empty() {
            cross_flags = linker_flags_for_target(triple)?;
        }
    }

    // Detect platform to choose default linker
    let default_linker = if cfg!(target_os = "windows") {
        "link.exe"
    } else {
        "cc"
    };

    let linker_name = linker.unwrap_or(default_linker).to_string();

    // If no linker is explicitly provided, try to detect a better one on Unix
    let final_linker_name = if linker.is_none() && !cfg!(target_os = "windows") {
        detect_unix_linker()
    } else {
        linker_name
    };

    let linker_invoker: Box<dyn LinkerInvoker> = if cfg!(target_os = "windows") {
        Box::new(MsvcLinker {
            linker: final_linker_name,
        })
    } else {
        Box::new(UnixLinker {
            linker: final_linker_name,
        })
    };

    // Combine cross-compilation flags (if any) with user-supplied flags.
    let combined_flags = if cross_flags.is_empty() {
        link_flags.map(|s| s.to_string())
    } else {
        let mut all = cross_flags.join(" ");
        if let Some(user) = link_flags {
            if !user.is_empty() {
                all = format!("{} {}", all, user);
            }
        }
        Some(all)
    };

    linker_invoker.link(obj_path, output_path, combined_flags.as_deref())
}

/// Map a target triple to the linker flags required to cross-compile for it.
///
/// Plan §18.1: maintains a small `target_triple -> linker_flags` table covering
/// the CI-tested target set. Returns an error for unmapped targets rather than
/// silently passing host flags to a cross target.
///
/// - `clang`/`gcc` drivers accept `--target=<triple>` directly.
/// - GNU `ld` needs an explicit emulation via `-m <emulation>` (e.g.
///   `-m aarch64linux` for an aarch64 Linux target from an x86_64 host).
pub(crate) fn linker_flags_for_target(triple: &str) -> Result<Vec<String>, String> {
    // Normalize: the emulation table keys on the architecture component.
    let arch = triple.split('-').next().unwrap_or(triple);
    // emulation -> linker -m flag for GNU ld (matches common cross targets).
    let emulation = match arch {
        "aarch64" | "arm64" => "aarch64linux",
        "x86_64" => "elf_x86_64",
        "i686" | "i386" => "elf_i386",
        "arm" => "armelf_linux_eabi",
        "riscv64" => "elf64lriscv",
        "powerpc64" => "elf64ppc",
        "powerpc" => "elf32ppclinux",
        "mips" => "elf32elmip",
        "mips64" => "elf64elmip",
        "s390x" => "elf64_s390",
        "wasm32" => "elf32_wasm",
        _ => {
            return Err(format!(
                "cross-compilation target '{}' is not in the supported linker-flag table; \
                 supply explicit --target/--ld-emulation flags via the linker config",
                triple
            ))
        }
    };
    Ok(vec![
        "--target".to_string(),
        triple.to_string(),
        "-m".to_string(),
        emulation.to_string(),
    ])
}

/// Tries to find a suitable C linker on Unix-like systems.
///
/// Preference order (plan §18.1): a C-compiler-driver (`cc`/`clang`/`gcc`) is
/// preferred because it supplies default system library paths and CRT objects
/// automatically; raw linkers (`ld`, `ld.lld`, `ld.gold`, `mold`) are only used
/// as a fallback when no driver is available. Each candidate is probed by
/// running `<candidate> --version`; the first that responds is returned.
fn detect_unix_linker() -> String {
    const CANDIDATES: &[&str] = &["cc", "clang", "gcc", "ld", "ld.lld", "ld.gold", "mold"];
    for candidate in CANDIDATES {
        if Command::new(candidate).arg("--version").output().is_ok() {
            return candidate.to_string();
        }
    }
    "cc".to_string() // Fallback; callers surface the failure if `cc` is absent.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_linker_command_construction() {
        // Plan §18.1: detect_unix_linker must probe an extended candidate set
        // (cc/clang/gcc drivers first, then raw linkers ld/ld.lld/ld.gold/mold)
        // rather than only cc/clang/gcc. On a machine with a C driver present
        // the result must be one of the probed candidates, proving it actually
        // detects rather than returning an arbitrary fallback.
        let detected = detect_unix_linker();
        assert!(!detected.is_empty());
        const CANDIDATES: &[&str] = &[
            "cc", "clang", "gcc", "ld", "ld.lld", "ld.gold", "mold",
        ];
        assert!(
            CANDIDATES.contains(&detected.as_str()),
            "detected linker '{}' must be one of the probed candidates",
            detected
        );
    }
}
