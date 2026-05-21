use std::sync::Arc;
use glyim_test::harness::compiler::{PipelineCompiler, TestCompiler, CompileOutput};
use glyim_test::mock::MockCodegen;
use glyim_test::assert_no_errors;
use glyim_test::assert_has_errors;
use glyim_test::assert_diag_contains;
use glyim_span::FileId;

fn compile(src: &str) -> CompileOutput {
    let backend = Arc::new(MockCodegen::new());
    let compiler = PipelineCompiler::new(backend);
    compiler.compile(src, FileId::from_raw(1), &[])
}

fn print_diagnostics(output: &CompileOutput) {
    if !output.diagnostics.is_empty() {
        eprintln!("=== DIAGNOSTICS ===");
        for diag in &output.diagnostics {
            eprintln!("{:?}", diag);
        }
        eprintln!("==================");
    }
}

#[test]
fn match_guard_uses_binding() {
    let output = compile(
        r#"
        fn main() {
            let x = Some(5);
            match x {
                Some(y) if y > 0 => {},
                _ => {}
            }
        }
        "#,
    );
    if !output.diagnostics.is_empty() {
        print_diagnostics(&output);
    }
    assert_no_errors(&output.diagnostics);
}

#[test]
fn or_pattern_same_types() {
    let output = compile(
        r#"
        fn main() {
            match 1 {
                1 | 2 => {},
                _ => {}
            }
        }
        "#,
    );
    if !output.diagnostics.is_empty() {
        print_diagnostics(&output);
    }
    assert_no_errors(&output.diagnostics);
}

#[test]
fn range_pattern_integer() {
    let output = compile(
        r#"
        fn main() {
            match 5 {
                1..=10 => {},
                _ => {}
            }
        }
        "#,
    );
    if !output.diagnostics.is_empty() {
        print_diagnostics(&output);
    }
    assert_no_errors(&output.diagnostics);
}

#[test]
fn slice_pattern_array() {
    let output = compile(
        r#"
        fn main() {
            let arr = [1, 2, 3];
            match arr {
                [a, b, ..rest] => {},
                _ => {}
            }
        }
        "#,
    );
    if !output.diagnostics.is_empty() {
        print_diagnostics(&output);
    }
    assert_no_errors(&output.diagnostics);
}

#[test]
fn index_expression_array() {
    let output = compile(
        r#"
        fn main() {
            let arr = [1, 2, 3];
            let x = arr[0];
        }
        "#,
    );
    if !output.diagnostics.is_empty() {
        print_diagnostics(&output);
    }
    assert_no_errors(&output.diagnostics);
}

#[test]
fn struct_literal_with_spread() {
    let output = compile(
        r#"
        struct S { x: i32, y: i32 }
        fn main() {
            let a = S { x: 1, y: 2 };
            let b = S { x: 3, ..a };
        }
        "#,
    );
    if !output.diagnostics.is_empty() {
        print_diagnostics(&output);
    }
    assert_no_errors(&output.diagnostics);
}

#[test]
fn or_pattern_mismatched_types_fails() {
    let output = compile(
        r#"
        fn main() {
            match 1 {
                true | "hello" => {},
                _ => {}
            }
        }
        "#,
    );
    if !output.diagnostics.is_empty() {
        print_diagnostics(&output);
    }
    assert_has_errors(&output.diagnostics);
    assert_diag_contains(&output.diagnostics, "mismatched types");
}
