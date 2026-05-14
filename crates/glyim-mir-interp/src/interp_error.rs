#[derive(Debug, PartialEq)]
pub enum InterpError {
    TimedOut,
    StackOverflow,
    Panic(String),
}

impl std::fmt::Display for InterpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimedOut => write!(f, "interpreter timed out"),
            Self::StackOverflow => write!(f, "stack overflow"),
            Self::Panic(msg) => write!(f, "panic: {}", msg),
        }
    }
}

impl std::error::Error for InterpError {}
