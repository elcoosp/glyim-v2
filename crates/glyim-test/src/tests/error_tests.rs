use crate::*;

#[test]
fn test_failure_reason_display() {
    let reason = error::FailureReason::TimeoutExceeded { timeout_secs: 30 };
    assert!(reason.to_string().contains("30"));
    let reason = error::FailureReason::ErrorPatternNotFound {
        pattern: "test".into(),
    };
    assert!(reason.to_string().contains("test"));
}

#[test]
fn test_timeout_error() {
    let err = error::TimeoutError { timeout_secs: 60 };
    assert!(err.to_string().contains("60"));
}

#[test]
fn test_failure_reason_run_failed_display() {
    let reason = error::FailureReason::RunFailed {
        exit_code: Some(139),
        expected_exit_code: Some(0),
    };
    assert!(reason.to_string().contains("139"));
    let reason = error::FailureReason::StdoutMismatch {
        expected: "hello".to_string(),
        actual: "world".to_string(),
    };
    assert!(reason.to_string().contains("hello"));
    let reason = error::FailureReason::StderrMismatch {
        expected: "error".to_string(),
        actual: "warning".to_string(),
    };
    assert!(reason.to_string().contains("error"));
    let reason = error::FailureReason::RunTimeout { timeout_secs: 30 };
    assert!(reason.to_string().contains("30"));
}
