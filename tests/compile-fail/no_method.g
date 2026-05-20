struct Foo;
fn main() {
    let f = Foo;
    f.bar(); //~ ERROR no method `bar` found
}
