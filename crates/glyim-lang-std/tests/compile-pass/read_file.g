// test-mode: compile-pass
// V18-T02: Read a file from disk

fn main() {
    let content = fs::read_to_string("test.txt");
    match content {
        Result::Ok(s) => println!("{}", s),
        Result::Err(e) => eprintln!("error: {}", e.message()),
    }
}
