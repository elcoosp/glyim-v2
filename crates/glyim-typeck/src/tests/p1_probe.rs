//! Real-pipeline probes for remaining P1 gaps. Modeled on the proven-working
//! `block_on<F: Future> -> F::Output` pattern in async_await.rs.
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
fn p1a_concrete_impl_output_projection() {
    // Non-generic impl so the projection's self type is concrete (no inference
    // var). Confirms impl-param substitution normalizes `F::Output` to `i32`.
    let src = r#"
        enum Poll<T> { Ready(T), Pending }
        trait Future {
            type Output;
            fn poll(&mut self) -> Poll<Self::Output>;
        }
        fn block_on<F: Future>(mut f: F) -> F::Output {
            loop {
                match f.poll() {
                    Poll::Ready(v) => return v,
                    Poll::Pending => { }
                }
            }
        }
        struct ReadyInt { val: i32 }
        impl Future for ReadyInt {
            type Output = i32;
            fn poll(&mut self) -> Poll<i32> { Poll::Ready(self.val) }
        }
        fn main() -> i32 {
            let r = ReadyInt { val: 7 };
            block_on(r)
        }
    "#;
    let output = compile(src);
    for d in &output.diagnostics {
        eprintln!("DIAG: {:?}", d.message);
    }
    assert_no_errors(&output.diagnostics);
}

#[test]
#[ignore = "KNOWN GAP (p1a): generic impl<T> method bodies do not yet scope the \
impl's type param T into the body (unresolved type T), so the associated-type \
projection normalizes (F::Output -> i32) but the impl method body fails to \
type-check. Concrete (non-generic) impl projection is fixed and covered above."]
fn p1a_generic_impl_concrete_arg_projection() {
    // Generic impl `impl<T> Future for ReadyT<T>` with a concrete `ReadyT<i32>`
    // argument. Requires impl-param substitution (T := i32) during projection
    // normalization.
    let src = r#"
        enum Poll<T> { Ready(T), Pending }
        trait Future {
            type Output;
            fn poll(&mut self) -> Poll<Self::Output>;
        }
        fn block_on<F: Future>(mut f: F) -> F::Output {
            loop {
                match f.poll() {
                    Poll::Ready(v) => return v,
                    Poll::Pending => { }
                }
            }
        }
        struct ReadyT<T> { val: T }
        impl<T> Future for ReadyT<T> {
            type Output = T;
            fn poll(&mut self) -> Poll<T> { Poll::Ready(self.val) }
        }
        fn make() -> ReadyT<i32> { ReadyT { val: 7 } }
        fn main() -> i32 {
            let r = make();
            block_on(r)
        }
    "#;
    let output = compile(src);
    for d in &output.diagnostics {
        eprintln!("DIAG: {:?}", d.message);
    }
    assert_no_errors(&output.diagnostics);
}
