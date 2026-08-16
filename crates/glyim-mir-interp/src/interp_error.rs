#[derive(Debug, PartialEq)]
pub enum InterpError {
    TimedOut,
    StackOverflow,
    /// Integer (or float) division / remainder by zero (de-stubbing plan §11.2).
    DivisionByZero,
    Panic(String),
}

impl std::fmt::Display for InterpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimedOut => write!(f, "interpreter timed out"),
            Self::StackOverflow => write!(f, "stack overflow"),
            Self::DivisionByZero => write!(f, "attempt to calculate remainder/division with a divisor of zero"),
            Self::Panic(msg) => write!(f, "panic: {}", msg),
        }
    }
}

impl std::error::Error for InterpError {}
