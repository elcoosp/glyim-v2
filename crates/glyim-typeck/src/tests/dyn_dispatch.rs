//! Trait-method static dispatch: `Trait::method(receiver, ..)` resolves to the
//! concrete impl function selected by the receiver's type (no vtable / dyn
//! object needed for concrete receivers).

use glyim_span::FileId;
use glyim_test::assert_no_errors;
use glyim_test::harness::compiler::{CompileOutput, PipelineCompiler, TestCompiler};
use glyim_test::mock::MockCodegen;
use std::sync::Arc;

fn compile(src: &str) -> CompileOutput {
    let backend = Arc::new(MockCodegen::new());
    let compiler = PipelineCompiler::new(backend);
    compiler.compile(src, FileId::from_raw(1), &[])
}

#[test]
fn trait_method_path_dispatch_resolves() {
    let src = r#"
        trait Animal { fn speak(&self) -> i32; }
        struct Dog;
        struct Cat;
        impl Animal for Dog { fn speak(&self) -> i32 { 1 } }
        impl Animal for Cat { fn speak(&self) -> i32 { 2 } }
        fn main() -> i32 { let d = Dog; let c = Cat; Animal::speak(&d) + Animal::speak(&c) }
    "#;
    let output = compile(src);
    assert_no_errors(&output.diagnostics);
}

#[test]
fn unit_struct_construction_resolves() {
    let src = r#"
        struct Dog;
        struct Cat;
        fn id(x: Dog) -> Dog { x }
        fn main() -> i32 { let d = Dog; let _ = id(d); 0 }
    "#;
    let output = compile(src);
    assert_no_errors(&output.diagnostics);
}

/// Probe the `async fn` desugaring *target* through the real pipeline:
/// a trait with an associated type + method, an impl providing it, and a
/// generic `block_on<F: MyFuture>` that drives it.
///
/// `#[ignore]`d because this currently FAILS (8 diagnostics) — the compiler
/// does not yet monomorphize generic function bodies (`block_on<F>`'s body is
/// checked with the rigid param `F`, so `F` never unifies with the concrete
/// `AddOne` and `F::Output` associated-type projection is not normalized).
/// Value/struct/variant path resolution, enum generic-param fields, generic
/// call *return-type* instantiation, and enum-variant pattern matching all
/// land. The remaining gap is full generic-body monomorphization + associated
/// type projection normalization, the foundational blocker for P5 (async)
/// *below* the coroutine state-machine pass. This test locks in the current
/// blocked behavior; remove `#[ignore]` and flip the assertion to
/// `assert_no_errors` once generic-body monomorphization + associated-type
/// projection land. See `KNOWN_GAPS.md` Phase 5.
#[test]
#[ignore = "P5 async: generic fn-body monomorphization + associated-type projection not yet resolved in typeck"]
fn async_desugar_target_compiles() {
    let src = r#"
        enum Poll<T> { Ready(T), Pending }
        trait MyFuture {
            type Output;
            fn poll(&mut self) -> Poll<Self::Output>;
        }
        struct AddOne { x: i32 }
        impl MyFuture for AddOne {
            type Output = i32;
            fn poll(&mut self) -> Poll<i32> { Poll::Ready(self.x + 1) }
        }
        fn block_on<F: MyFuture>(mut f: F) -> F::Output {
            loop {
                match f.poll() {
                    Poll::Ready(v) => return v,
                    Poll::Pending => { }
                }
            }
        }
        fn main() -> i32 { let f = AddOne { x: 41 }; block_on(f) }
    "#;
    let output = compile(src);
    // Characterization of the current blocker (8 diagnostics rooted in
    // generic fn-body monomorphization + associated-type projection).
    assert!(!output.diagnostics.is_empty(), "expected P5 blocker diagnostics");
}
