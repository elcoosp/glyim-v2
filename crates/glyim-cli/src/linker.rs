use std::path::Path;

pub fn invoke_linker(obj_path: &Path, output_path: &Path) -> Result<(), String> {
    let status = std::process::Command::new("cc")
        .arg(obj_path.as_os_str())
        .arg("-o")
        .arg(output_path.as_os_str())
        .status()
        .map_err(|e| format!("Failed to invoke linker: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err("Linker failed".to_string())
    }
}
