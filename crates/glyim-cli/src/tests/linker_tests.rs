#[cfg(test)]
mod tests {
    use crate::linker::invoke_linker;
    use std::path::PathBuf;

    #[test]
    fn test_linker_fails_on_nonexistent_obj() {
        let obj = PathBuf::from("nonexistent.o");
        let out = PathBuf::from("output");
        let result = invoke_linker(&obj, &out);
        assert!(result.is_err());
    }

    // We cannot easily test success because it requires a real object file and cc.
}
