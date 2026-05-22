fn main(arr: [i32; 3]) -> i32 {
    match arr {
        [a, b] => a + b,
        _ => 0
    }
}
