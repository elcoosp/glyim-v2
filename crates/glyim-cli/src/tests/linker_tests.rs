use crate::linker::invoke_linker;
use std::path::PathBuf;

#[test]
fn test_invoke_linker_basic() {
    let obj = PathBuf::from("dummy.o");
    let out = PathBuf::from("dummy_out");
    let result = invoke_linker(&obj, &out, None, None);
    let _ = result;
}
