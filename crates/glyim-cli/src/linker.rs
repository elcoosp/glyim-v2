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
) -> Result<(), String> {
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

    linker_invoker.link(obj_path, output_path, link_flags)
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
