// test-mode: compile-pass
// V18-T05: Environment variables

fn main() {
    let val = env::var("HOME");
    match val {
        Result::Ok(home) => println!("HOME={}", home),
        Result::Err(e) => eprintln!("error: {}", e),
    }
    env::set_var("GLYIM_TEST", "hello");
    let args = env::args();
}
