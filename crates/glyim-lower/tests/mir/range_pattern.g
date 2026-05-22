fn main(x: i32) -> bool {
    match x {
        0..=9 => true,
        _ => false
    }
}
