use std::path::Path;

pub fn invoke_linker(
    obj_path: &Path,
    output_path: &Path,
    linker: Option<&str>,
    link_flags: Option<&str>,
) -> Result<(), String> {
    // Default to `cc` on Unix-like systems, `link.exe` on Windows.
    // This can be overridden by the `--linker` CLI flag.
    let default_linker = if cfg!(target_os = "windows") {
        "link.exe"
    } else {
        "cc"
    };

    let linker = linker.unwrap_or(default_linker);
    let mut cmd = std::process::Command::new(linker);

    // Platform-specific default flags
    if cfg!(target_os = "windows") {
        // MSVC defaults
        cmd.arg(obj_path.as_os_str())
           .arg("/OUT:")
           .arg(output_path.as_os_str());
    } else {
        // GCC/Clang defaults
        cmd.arg(obj_path.as_os_str())
           .arg("-o")
           .arg(output_path.as_os_str());
    }

    if let Some(flags) = link_flags {
        for flag in flags.split_whitespace() {
            cmd.arg(flag);
        }
    }

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to invoke linker '{}': {}", linker, e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("Linker '{}' failed with status {}", linker, status))
    }
}
