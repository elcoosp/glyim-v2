// S01-T03: Calling a stubbed function at runtime panics with unimplemented!
#[test]
#[should_panic(expected = "not implemented")]
fn test_stub_macro_panics() {
    fn stub_function() {
        stub!("This is a stub");
    }
    stub_function();
}

#[test]
#[should_panic(expected = "not implemented")]
fn test_stub_impl_macro_panics() {
    fn stub_impl_function() {
        stub_impl!("Implementation stub");
    }
    stub_impl_function();
}

#[test]
#[should_panic(expected = "not implemented")]
fn test_stub_macro_format_args() {
    fn stub_with_format() {
        let reason = "format test";
        stub!("Reason: {}", reason);
    }
    stub_with_format();
}
