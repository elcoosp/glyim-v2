//! Pipeline-level proof that generic functions are supported end-to-end
//! (plan §9.A). A `fn id<T>(x: T) -> T` must resolve the type parameter `T`
//! (previously `unresolved type T`, because HIR lowering dropped the
//! `TypeParamList` produced by the parser) and the generic body must
//! type-check and lower through the pipeline.
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
fn generic_fn_typechecks_and_lowers() {
    let src = r#"
        fn id<T>(x: T) -> T { x }
        fn main() -> i32 {
            let a = id(40);
            let b = id(2);
            a + b
        }
    "#;
    let output = compile(src);
    assert_no_errors(&output.diagnostics);
}
