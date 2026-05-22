// ignore - requires full typeck/frontend (S03)
fn main(x: Option<i32>) -> i32 {
    match x {
        Some(y) if y > 0 => y,
        _ => 0
    }
}
