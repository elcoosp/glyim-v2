// ignore
// requires full typeck/frontend (S03): array-length inference in slice patterns
fn main(arr: [i32; 3]) -> i32 {
    match arr {
        [a, b] => a + b,
        _ => 0
    }
}
