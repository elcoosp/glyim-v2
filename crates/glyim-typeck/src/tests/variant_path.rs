//! Pipeline-level proof that an enum variant defined in the value namespace is
//! referenceable through a value-namespace path (`Color::Red` and `mod::Color::Red`).
//!
//! This exercises: def-map registration of enum variants in the value
//! namespace, `check_path` resolving the variant to a `thir::ExprKind::VariantRef`
//! carrying the enclosing enum's type, and MIR lowering to an `Aggregate` of the
//! ADT. Data-carrying variant constructors (function-typed) are a follow-up.
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
fn unit_variant_referenced_by_single_segment_path() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() {
            let _ = Color::Red;
            let _ = Color::Green;
        }
    "#;
    let output = compile(src);
    assert_no_errors(&output.diagnostics);
}

#[test]
fn unit_variant_referenced_by_multi_segment_path() {
    let src = r#"
        mod palette {
            enum Color { Red, Green }
        }
        fn main() {
            let _ = palette::Color::Red;
        }
    "#;
    let output = compile(src);
    assert_no_errors(&output.diagnostics);
}
