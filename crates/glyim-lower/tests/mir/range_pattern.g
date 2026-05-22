// ignore - requires full typeck/frontend (S03)
fn main(x: i32) -> bool {
    match x {
        0..=9 => true,
        _ => false
    }
}
