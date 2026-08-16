//! Frontend: lexer + parser merged.
#![allow(missing_docs)]
pub mod lexer;
pub mod parser;

pub use lexer::{LexResult, Token, lex};
pub use parser::{ParseResult, parse_to_syntax, try_parse_fragment};

#[cfg(test)]
mod tests;
