// test-mode: run-pass
fn main() {
    let _y: <<i32 as Iterator>::Item as OtherTrait>::Target = 10u32;
}
