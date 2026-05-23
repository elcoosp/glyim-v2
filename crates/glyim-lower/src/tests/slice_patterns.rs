use std::fs;
use tempfile::TempDir;
use glyim_test::mock::TestDbBuilder;

fn compile_slice_pattern(src: &str) -> Result<(), Vec<glyim_diag::GlyimDiagnostic>> {
    let temp_dir = TempDir::new().unwrap();
    let source_path = temp_dir.path().join("test.g");
    fs::write(&source_path, src).unwrap();
    let output_path = temp_dir.path().join("out.o");
    let mut db = TestDbBuilder::new()
        .name("slice_test")
        .target_triple("x86_64-unknown-linux-gnu")
        .opt_level(0)
        .file(source_path.clone(), src)
        .build();
    let backend = glyim_codegen_llvm::LlvmBackend::new();
    glyim_pipeline::Pipeline::compile_file(&mut db, &source_path, &backend, &output_path)
}

#[test]
fn slice_pattern_array() {
    let src = r#"
    fn main() {
        let arr = [1, 2, 3];
        match arr {
            [a, b, c] => (),
            _ => (),
        }
    }
    "#;
    match compile_slice_pattern(src) {
        Ok(()) => (),
        Err(diags) => {
            for d in diags {
                eprintln!("ERROR: {}", d.message);
            }
            panic!("compilation failed");
        }
    }
}

#[test]
fn slice_pattern_slice_reference() {
    let src = r#"
    fn main() {
        let arr = [1, 2, 3, 4];
        match &arr[..] {
            [a, b, .., c] => (),
            _ => (),
        }
    }
    "#;
    match compile_slice_pattern(src) {
        Ok(()) => (),
        Err(diags) => {
            for d in diags {
                eprintln!("ERROR: {}", d.message);
            }
            panic!("compilation failed");
        }
    }
}
