//! Tests for method resolution: method calls on types with trait-based lookup.

use glyim_core::interner::Name;
use glyim_core::primitives::{IntTy, Mutability};
use glyim_hir::{BodyId, Expr, ExprId, ItemKind, Param, TypeRef};
use glyim_span::Span;
use glyim_test::{
    assert_ty, test_ty_ctx, with_fresh_ty_ctx, AnalysisTester, MockSolver,
    fixtures::TyFactory,
};
use glyim_type::{Ty, TyCtx};

use crate::typeck_crate;

// Helper to create a minimal HIR for method call testing.
// Instead of building full HIR manually, we use AnalysisTester which can
// parse source and run type checking.
// But since we're testing method resolution specifically, we create simple
// test cases that exercise the method call path.

#[test]
fn test_method_call_on_vec_push() {
    // We'll use a source file that defines a struct Vec<T> with method push.
    // Because our type checker currently does not have trait-based method
    // resolution fully implemented, we will rely on the `resolve_method_call`
    // stub which currently looks up impls in the HIR.
    // To make this test pass, we must implement proper trait resolution.
    // The test will initially fail, then we implement.
    let source = r#"
        struct Vec<T> { data: *mut T, len: usize, cap: usize }
        impl<T> Vec<T> {
            fn push(&mut self, value: T) { }
        }
        fn main() {
            let mut v = Vec::<i32> { data: 0 as *mut i32, len: 0, cap: 0 };
            v.push(42);
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert!(result.diagnostics.is_empty(), "typeck failed: {:?}", result.diagnostics);
    // Find the method call expression and check its type.
    // For now we just verify that no errors occurred.
}

#[test]
fn test_method_call_with_multiple_args() {
    let source = r#"
        struct Point { x: i32, y: i32 }
        impl Point {
            fn translate(&mut self, dx: i32, dy: i32) { self.x += dx; self.y += dy; }
        }
        fn main() {
            let mut p = Point { x: 0, y: 0 };
            p.translate(5, -3);
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert!(result.diagnostics.is_empty());
}

#[test]
fn test_method_call_no_method() {
    let source = r#"
        struct Foo;
        fn main() {
            let f = Foo;
            f.bar(); //~ ERROR no method `bar` found
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert!(!result.diagnostics.is_empty());
    let has_error = result.diagnostics.iter().any(|d| d.message.contains("no method `bar`"));
    assert!(has_error, "Expected method not found error");
}

#[test]
fn test_method_call_with_self_by_value() {
    let source = r#"
        struct Number { val: i32 }
        impl Number {
            fn get(self) -> i32 { self.val }
        }
        fn main() {
            let n = Number { val: 10 };
            let x = n.get();
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert!(result.diagnostics.is_empty());
}

#[test]
fn test_method_call_with_trait() {
    // This test requires trait resolution. We'll use a mock solver initially.
    // The full implementation will use real trait solver.
    let source = r#"
        trait Greet { fn greet(&self) -> String; }
        struct Person { name: String }
        impl Greet for Person {
            fn greet(&self) -> String { self.name.clone() }
        }
        fn main() {
            let p = Person { name: "Alice".to_string() };
            let msg = p.greet();
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    // Use mock solver that returns Proven for any predicate (simplified)
    let mock_solver = MockSolver::new().respond_for_any(glyim_solve::SolverResult::Proven);
    let result = tester.with_solver(Box::new(mock_solver)).typeck();
    assert!(result.diagnostics.is_empty());
}
