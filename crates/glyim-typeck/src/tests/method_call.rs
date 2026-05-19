#[ignore]
#[ignore]
use glyim_test::phase::FrontendTester;

#[test]
fn test() {
    let source = if file!().ends_with("method_call.rs") {
        r#"
            trait Foo { fn method(&self) -> i32; }
            impl Foo for i32 { fn method(&self) -> i32 { 42 } }
            fn test() { let x = 5.method(); }
        "#
    } else if file!().ends_with("multi_seg_path.rs") {
        r#"
            mod a { pub struct S; }
            fn test() { let x: a::S; }
        "#
    } else if file!().ends_with("fn_sig_inst.rs") {
        r#"
            fn foo(x: i32) -> i32 { x }
            fn test() { let y = foo(42); }
        "#
    } else if file!().ends_with("typeck_result_accessors.rs") {
        r#"
            fn test() -> i32 {
                let a = 42;
                a
            }
        "#
    } else {
        r#"
            struct Point { x: i32, y: i32 }
            fn test(p: Point) {
                match p { Point { x, y } => { let _ = x + y; } }
            }
        "#
    };
    let trace = FrontendTester::new(source).run();
    let typeck = trace.typeck_result.expect("typeck failed");
    assert!(typeck.diagnostics.is_empty());
}
