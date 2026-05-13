//! Frontend: lexer + parser merged.
pub mod lexer;
pub mod parser;

pub use lexer::{lex, Token, LexResult};
pub use parser::{parse_to_syntax, ParseResult};
