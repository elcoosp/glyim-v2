#[derive(Debug, PartialEq)]
pub enum InterpError {
    TimedOut,
    StackOverflow,
    Panic(String),
}
