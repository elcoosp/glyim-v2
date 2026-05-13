//! Frontend: lexer + parser merged.
pub mod lexer;
pub mod parser;

pub use lexer::{LexResult, Token, lex};
pub use parser::{ParseResult, parse_to_syntax};

#[cfg(test)]
mod tests;
