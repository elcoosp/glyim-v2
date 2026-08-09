#[cfg(test)]
mod tests {
    use crate::CliArgs;
    use crate::run_with_args;
    use std::path::PathBuf;

    #[test]
    fn test_unknown_emit_returns_error() {
        let args = CliArgs {
            input: PathBuf::from("test.g"),
            output: None,
            emit: "unknown".to_string(),
            opt_level: 0,
            target: None,
            backend: "llvm".to_string(),
        };
        let result = run_with_args(args);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(!errs.is_empty());
        assert!(errs[0].message.contains("unknown emit type"));
    }

    #[test]
    fn test_emit_mir_writes_file() {
        let output = PathBuf::from("test_output.mir");
        let args = CliArgs {
            input: PathBuf::from("test.g"),
            output: Some(output.clone()),
            emit: "mir".to_string(),
            opt_level: 0,
            target: None,
            backend: "llvm".to_string(),
        };
        let result = run_with_args(args);
        assert!(result.is_ok());
        assert!(output.exists());
        std::fs::remove_file(&output).ok();
    }

    #[test]
    fn test_emit_llvm_ir_writes_file() {
        let output = PathBuf::from("test_output.ll");
        let args = CliArgs {
            input: PathBuf::from("test.g"),
            output: Some(output.clone()),
            emit: "llvm-ir".to_string(),
            opt_level: 0,
            target: None,
            backend: "llvm".to_string(),
        };
        let result = run_with_args(args);
        assert!(result.is_ok());
        assert!(output.exists());
        std::fs::remove_file(&output).ok();
    }
}
