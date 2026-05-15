// test-mode: run-pass
fn foo<T>()
where
    T::Item: Clone,
{
}
fn main() {
    foo::<i32>();
}
