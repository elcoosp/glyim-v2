use crate::*;

#[test]
fn test_config_parsing() {
    let source = "// test-mode: compile-fail\n// error-pattern: mismatched types\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert!(result.has_explicit_mode);
    assert_eq!(result.config.mode, harness::config::TestMode::CompileFail);
    assert_eq!(result.config.error_patterns.len(), 1);
    assert_eq!(result.config.error_patterns[0], "mismatched types");
}

#[test]
fn test_config_default_mode() {
    let source = "fn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert!(!result.has_explicit_mode);
    assert_eq!(result.config.mode, harness::config::TestMode::CompilePass);
}

#[test]
fn test_config_ignore() {
    let source = "// ignore\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert!(result.config.ignore);
}

#[test]
fn test_config_timeout() {
    let source = "// timeout: 120\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert_eq!(result.config.timeout_secs, 120);
}

#[test]
fn test_config_compile_flags() {
    let source = "// compile-flags: --emit=mir -Zdump-mir=all\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert_eq!(result.config.compile_flags.len(), 2);
}

#[test]
fn test_config_revisions() {
    let source = "// revisions: a b c\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert_eq!(result.config.revisions, vec!["a", "b", "c"]);
}

#[test]
fn test_config_revision_flags() {
    let source = "// revisions: a b\n//[a] compile-flags: -Dwarnings\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert!(result.config.revision_compile_flags.contains_key("a"));
}

#[test]
fn test_test_mode_from_str() {
    assert_eq!(
        "compile-pass".parse::<harness::config::TestMode>().unwrap(),
        harness::config::TestMode::CompilePass
    );
    assert_eq!(
        "compile-fail".parse::<harness::config::TestMode>().unwrap(),
        harness::config::TestMode::CompileFail
    );
    assert_eq!(
        "ui".parse::<harness::config::TestMode>().unwrap(),
        harness::config::TestMode::Ui
    );
    assert!("invalid".parse::<harness::config::TestMode>().is_err());
}

#[test]
fn test_test_mode_dir_name() {
    assert_eq!(
        harness::config::TestMode::CompilePass.dir_name(),
        "compile-pass"
    );
    assert_eq!(
        harness::config::TestMode::CompileFail.dir_name(),
        "compile-fail"
    );
    assert_eq!(harness::config::TestMode::Ui.dir_name(), "ui");
}

#[test]
fn test_run_pass_mode_from_str() {
    assert_eq!(
        "run-pass".parse::<harness::config::TestMode>().unwrap(),
        harness::config::TestMode::RunPass
    );
    assert_eq!(
        "run-fail".parse::<harness::config::TestMode>().unwrap(),
        harness::config::TestMode::RunFail
    );
}

#[test]
fn test_run_pass_mode_dir_name() {
    assert_eq!(harness::config::TestMode::RunPass.dir_name(), "run-pass");
    assert_eq!(harness::config::TestMode::RunFail.dir_name(), "run-fail");
}

#[test]
fn test_config_check_stdout() {
    let source = "// test-mode: run-pass\n// check-stdout: Hello, world!\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert_eq!(result.config.mode, harness::config::TestMode::RunPass);
    assert_eq!(result.config.check_stdout.as_deref(), Some("Hello, world!"));
}

#[test]
fn test_config_check_stderr() {
    let source = "// test-mode: run-fail\n// check-stderr: panic\n// exit-code: 101\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert_eq!(result.config.mode, harness::config::TestMode::RunFail);
    assert_eq!(result.config.check_stderr.as_deref(), Some("panic"));
    assert_eq!(result.config.expected_exit_code, Some(101));
}

#[test]
fn test_output_check_exit_code_pass() {
    let check = harness::runner::OutputCheck::new().exit_code(0);
    let result = harness::runner::RunResult {
        exit_code: Some(0),
        stdout: String::new(),
        stderr: String::new(),
        timed_out: false,
        duration: std::time::Duration::from_secs(0),
    };
    assert!(check.check(&result).is_ok());
}

#[test]
fn test_output_check_exit_code_fail() {
    let check = harness::runner::OutputCheck::new().exit_code(0);
    let result = harness::runner::RunResult {
        exit_code: Some(1),
        stdout: String::new(),
        stderr: String::new(),
        timed_out: false,
        duration: std::time::Duration::from_secs(0),
    };
    assert!(check.check(&result).is_err());
}

#[test]
fn test_output_check_stdout_pass() {
    let check = harness::runner::OutputCheck::new().stdout("hello");
    let result = harness::runner::RunResult {
        exit_code: Some(0),
        stdout: "say hello world".to_string(),
        stderr: String::new(),
        timed_out: false,
        duration: std::time::Duration::from_secs(0),
    };
    assert!(check.check(&result).is_ok());
}

#[test]
fn test_output_check_stdout_fail() {
    let check = harness::runner::OutputCheck::new().stdout("goodbye");
    let result = harness::runner::RunResult {
        exit_code: Some(0),
        stdout: "say hello world".to_string(),
        stderr: String::new(),
        timed_out: false,
        duration: std::time::Duration::from_secs(0),
    };
    assert!(check.check(&result).is_err());
}

#[test]
fn test_output_check_stderr_pass() {
    let check = harness::runner::OutputCheck::new().stderr("error:");
    let result = harness::runner::RunResult {
        exit_code: Some(1),
        stdout: String::new(),
        stderr: "error: something went wrong".to_string(),
        timed_out: false,
        duration: std::time::Duration::from_secs(0),
    };
    assert!(check.check(&result).is_ok());
}

#[test]
fn test_output_check_timeout() {
    let check = harness::runner::OutputCheck::new().exit_code(0);
    let result = harness::runner::RunResult {
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        timed_out: true,
        duration: std::time::Duration::from_secs(60),
    };
    let err = check.check(&result).unwrap_err();
    assert!(matches!(err, error::FailureReason::RunTimeout { .. }));
}

#[test]
fn test_output_check_run_fail_pass() {
    let check = harness::runner::OutputCheck::new()
        .exit_code(1)
        .stderr("panic");
    let result = harness::runner::RunResult {
        exit_code: Some(1),
        stdout: String::new(),
        stderr: "thread panicked: panic at core.rs:42".to_string(),
        timed_out: false,
        duration: std::time::Duration::from_millis(100),
    };
    assert!(check.check(&result).is_ok());
}

#[test]
fn test_program_runner_nonexistent() {
    let runner = harness::runner::ProgramRunner::new("/nonexistent/program");
    let result = runner.run(std::time::Duration::from_secs(5));
    assert!(result.exit_code.is_none());
    assert!(!result.stderr.is_empty());
}

#[test]
fn test_program_runner_echo() {
    let echo_path = "/bin/echo";
    let runner = harness::runner::ProgramRunner::new(echo_path).arg("hello world");
    let result = runner.run(std::time::Duration::from_secs(5));
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, Some(0));
    assert!(result.stdout.contains("hello world"));
}

#[test]
fn test_program_runner_false() {
    let false_path = if cfg!(target_os = "macos") {
        "/usr/bin/false"
    } else {
        "/bin/false"
    };
    let runner = harness::runner::ProgramRunner::new(false_path);
    let result = runner.run(std::time::Duration::from_secs(5));
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, Some(1));
}

#[test]
fn test_program_runner_with_stdin() {
    let cat_path = "/bin/cat";
    let runner = harness::runner::ProgramRunner::new(cat_path).stdin("input data");
    let result = runner.run(std::time::Duration::from_secs(5));
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, Some(0));
    assert!(result.stdout.contains("input data"));
}
