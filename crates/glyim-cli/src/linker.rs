use std::path::Path;

pub fn invoke_linker(
    obj_path: &Path,
    output_path: &Path,
    linker: Option<&str>,
    link_flags: Option<&str>,
) -> Result<(), String> {
    let linker = linker.unwrap_or("cc");
    let mut cmd = std::process::Command::new(linker);
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
        .map_err(|e| format!("Failed to invoke linker: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err("Linker failed".to_string())
    }
}
