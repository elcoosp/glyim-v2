enum Option<T> { Some(T), None }
fn main() {
    let x = Option::Some(42);
    match x {
        Option::Some(0) | Option::None => { },
        Option::Some(n) => { let _ = n; },
    }
}
