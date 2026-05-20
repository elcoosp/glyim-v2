fn takes_immut(x: &i32) -> i32 { *x }
fn main() {
    let mut a = 5;
    let b = &mut a;
    let c = takes_immut(b);
}
