//! Pipeline-level proof that a constant defined in the value namespace is
//! referenceable through a value-namespace path (`CONST` and `mod::CONST`).
//!
//! This exercises: HIR `ConstItem` type registration in `typeck_crate`,
//! `check_path` resolving the const to a `thir::ExprKind::ConstRef` carrying
//! the constant's value type, and MIR lowering to `MirConstKind::ConstRef`.
use glyim_span::FileId;
use glyim_test::assert_no_errors;
use glyim_test::harness::compiler::{CompileOutput, PipelineCompiler, TestCompiler};
use std::sync::Arc;

use glyim_test::mock::MockCodegen;

fn compile(src: &str) -> CompileOutput {
    let backend = Arc::new(MockCodegen::new());
    let compiler = PipelineCompiler::new(backend);
    compiler.compile(src, FileId::from_raw(1), &[])
}

#[test]
fn top_level_const_referenced_by_path() {
    let src = r#"
        const ANSWER: i32 = 42;
        fn main() {
            let _ = ANSWER;
        }
    "#;
    let output = compile(src);
    assert_no_errors(&output.diagnostics);
}

#[test]
fn module_const_referenced_by_multi_segment_path() {
    let src = r#"
        mod config {
            const MAX: i32 = 100;
        }
        fn main() {
            let _ = config::MAX;
        }
    "#;
    let output = compile(src);
    assert_no_errors(&output.diagnostics);
}
