use glyim_diag::GlyimDiagnostic;
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_syntax::SyntaxKind;
use smol_str::SmolStr;

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: SyntaxKind,
    pub span: Span,
    pub text: SmolStr,
}

impl Token {
    pub fn new(kind: SyntaxKind, span: Span, text: impl AsRef<str>) -> Self {
        Self {
            kind,
            span,
            text: SmolStr::from(text.as_ref()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

pub struct Lexer<'a> {
    source: &'a str,
    file_id: FileId,
    pos: usize,
    diagnostics: Vec<GlyimDiagnostic>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str, file_id: FileId) -> Self {
        Self {
            source,
            file_id,
            pos: 0,
            diagnostics: Vec::new(),
        }
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span::new(
            self.file_id,
            ByteIdx::from_raw(start as u32),
            ByteIdx::from_raw(end as u32),
            SyntaxContext::ROOT,
        )
    }

    fn peek(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }
    fn peek_next(&self) -> Option<char> {
        let mut chars = self.source[self.pos..].chars();
        chars.next();
        chars.next()
    }
    fn advance(&mut self) -> Option<char> {
        let ch = self.source[self.pos..].chars().next();
        if let Some(c) = ch {
            self.pos += c.len_utf8();
        }
        ch
    }

    pub fn lex(mut self) -> LexResult {
        let mut tokens = Vec::new();

        while self.pos < self.source.len() {
            let start = self.pos;
            if self.lex_trivia() {
                continue;
            }
            if self.pos >= self.source.len() {
                break;
            }
            let Some(ch) = self.peek() else { break };

            match ch {
                'a'..='z' | 'A'..='Z' | '_' => {
                    self.lex_ident_or_keyword();
                    let text = &self.source[start..self.pos];
                    let kind = lookup_keyword(text);
                    tokens.push(Token::new(kind, self.span(start, self.pos), text));
                }
                '0'..='9' => {
                    let kind = self.lex_number();
                    let text = &self.source[start..self.pos];
                    tokens.push(Token::new(kind, self.span(start, self.pos), text));
                }
                '"' => {
                    self.lex_string();
                    let text = &self.source[start..self.pos];
                    tokens.push(Token::new(
                        SyntaxKind::StringLit,
                        self.span(start, self.pos),
                        text,
                    ));
                }
                '\'' => {
                    // Check for lifetime: 'identifier: (no closing quote before colon)
                    let saved_pos = self.pos;
                    // Peek ahead to see if it looks like a lifetime
                    self.pos += 1; // temporarily consume the quote
                    let mut is_lifetime = false;
                    let ident_start = self.pos;
                    // Read identifier
                    while let Some(ch) = self.peek() {
                        if ch.is_alphanumeric() || ch == '_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let has_ident = self.pos > ident_start;
                    if has_ident {
                        // Skip whitespace
                        while let Some(ch) = self.peek() {
                            if ch.is_whitespace() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        // Check for colon
                        if self.peek() == Some(':') {
                            is_lifetime = true;
                        }
                    }
                    // Restore position
                    self.pos = saved_pos;
                    if is_lifetime {
                        // Consume the quote and identifier
                        self.advance(); // skip '
                        while let Some(ch) = self.peek() {
                            if ch.is_alphanumeric() || ch == '_' {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        let text = &self.source[start..self.pos];
                        tokens.push(Token::new(
                            SyntaxKind::Lifetime,
                            self.span(start, self.pos),
                            text,
                        ));
                    } else {
                        self.lex_char();
                        let text = &self.source[start..self.pos];
                        tokens.push(Token::new(
                            SyntaxKind::CharLit,
                            self.span(start, self.pos),
                            text,
                        ));
                    }
                }
                '(' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::LParen,
                        self.span(start, self.pos),
                        "(",
                    ));
                }
                ')' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::RParen,
                        self.span(start, self.pos),
                        ")",
                    ));
                }
                '{' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::LBrace,
                        self.span(start, self.pos),
                        "{",
                    ));
                }
                '}' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::RBrace,
                        self.span(start, self.pos),
                        "}",
                    ));
                }
                '[' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::LBracket,
                        self.span(start, self.pos),
                        "[",
                    ));
                }
                ']' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::RBracket,
                        self.span(start, self.pos),
                        "]",
                    ));
                }
                ',' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::Comma,
                        self.span(start, self.pos),
                        ",",
                    ));
                }
                ';' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::Semicolon,
                        self.span(start, self.pos),
                        ";",
                    ));
                }
                '.' => {
                    let kind = self.lex_dot();
                    let text = &self.source[start..self.pos];
                    tokens.push(Token::new(kind, self.span(start, self.pos), text));
                }
                ':' => {
                    let kind = self.lex_colon();
                    let text = &self.source[start..self.pos];
                    tokens.push(Token::new(kind, self.span(start, self.pos), text));
                }
                '+' => {
                    let k = self.lex_plus();
                    tokens.push(Token::new(
                        k,
                        self.span(start, self.pos),
                        &self.source[start..self.pos],
                    ));
                }
                '-' => {
                    let k = self.lex_minus();
                    tokens.push(Token::new(
                        k,
                        self.span(start, self.pos),
                        &self.source[start..self.pos],
                    ));
                }
                '*' => {
                    let k = self.lex_star();
                    tokens.push(Token::new(
                        k,
                        self.span(start, self.pos),
                        &self.source[start..self.pos],
                    ));
                }
                '/' => {
                    let k = self.lex_slash();
                    tokens.push(Token::new(
                        k,
                        self.span(start, self.pos),
                        &self.source[start..self.pos],
                    ));
                }
                '%' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::Percent,
                        self.span(start, self.pos),
                        "%",
                    ));
                }
                '=' => {
                    let k = self.lex_eq();
                    tokens.push(Token::new(
                        k,
                        self.span(start, self.pos),
                        &self.source[start..self.pos],
                    ));
                }
                '!' => {
                    let k = self.lex_bang();
                    tokens.push(Token::new(
                        k,
                        self.span(start, self.pos),
                        &self.source[start..self.pos],
                    ));
                }
                '<' => {
                    let k = self.lex_lt();
                    tokens.push(Token::new(
                        k,
                        self.span(start, self.pos),
                        &self.source[start..self.pos],
                    ));
                }
                '>' => {
                    let k = self.lex_gt();
                    tokens.push(Token::new(
                        k,
                        self.span(start, self.pos),
                        &self.source[start..self.pos],
                    ));
                }
                '&' => {
                    let k = self.lex_and();
                    tokens.push(Token::new(
                        k,
                        self.span(start, self.pos),
                        &self.source[start..self.pos],
                    ));
                }
                '|' => {
                    let k = self.lex_or();
                    tokens.push(Token::new(
                        k,
                        self.span(start, self.pos),
                        &self.source[start..self.pos],
                    ));
                }
                '^' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::Caret,
                        self.span(start, self.pos),
                        "^",
                    ));
                }
                '@' => {
                    self.advance();
                    tokens.push(Token::new(SyntaxKind::At, self.span(start, self.pos), "@"));
                }
                '#' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::Hash,
                        self.span(start, self.pos),
                        "#",
                    ));
                }
                '$' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::Dollar,
                        self.span(start, self.pos),
                        "$",
                    ));
                }
                '~' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::Tilde,
                        self.span(start, self.pos),
                        "~",
                    ));
                }
                '?' => {
                    self.advance();
                    tokens.push(Token::new(
                        SyntaxKind::Question,
                        self.span(start, self.pos),
                        "?",
                    ));
                }
                _ => {
                    self.advance();
                    self.diagnostics.push(GlyimDiagnostic::lex_error(
                        self.span(start, self.pos),
                        format!("unexpected character: '{}'", ch),
                    ));
                    tokens.push(Token::new(
                        SyntaxKind::Error,
                        self.span(start, self.pos),
                        &self.source[start..self.pos],
                    ));
                }
            }
        }

        LexResult {
            tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn lex_trivia(&mut self) -> bool {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            match ch {
                ' ' | '\t' | '\n' | '\r' => {
                    self.advance();
                }
                '/' if self.peek_next() == Some('/') => {
                    self.advance();
                    self.advance();
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                '/' if self.peek_next() == Some('*') => {
                    self.advance();
                    self.advance();
                    let mut depth = 1u32;
                    while depth > 0 {
                        match self.peek() {
                            Some('/') if self.peek_next() == Some('*') => {
                                self.advance();
                                self.advance();
                                depth += 1;
                            }
                            Some('*') if self.peek_next() == Some('/') => {
                                self.advance();
                                self.advance();
                                depth -= 1;
                            }
                            Some(_) => {
                                self.advance();
                            }
                            None => break,
                        }
                    }
                }
                _ => break,
            }
        }
        self.pos > start
    }

    fn lex_ident_or_keyword(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn lex_number(&mut self) -> SyntaxKind {
        // Radix prefixes
        if self.peek() == Some('0') {
            let next = self.peek_next();
            if next == Some('x') || next == Some('X') {
                self.advance();
                self.advance();
                while let Some(ch) = self.peek() {
                    if ch.is_ascii_hexdigit() || ch == '_' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.lex_number_suffix();
                return SyntaxKind::IntLit;
            } else if next == Some('b') || next == Some('B') {
                self.advance();
                self.advance();
                while let Some(ch) = self.peek() {
                    if ch == '0' || ch == '1' || ch == '_' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.lex_number_suffix();
                return SyntaxKind::IntLit;
            } else if next == Some('o') || next == Some('O') {
                self.advance();
                self.advance();
                while let Some(ch) = self.peek() {
                    if ('0'..='7').contains(&ch) || ch == '_' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.lex_number_suffix();
                return SyntaxKind::IntLit;
            }
        }

        let mut is_float = false;

        // Decimal integer part
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }

        // Fractional part
        if self.peek() == Some('.') && self.peek_next().is_some_and(|n| n.is_ascii_digit()) {
            is_float = true;
            self.advance();
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() || ch == '_' {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Exponent part with validation
        if self.peek() == Some('e') || self.peek() == Some('E') {
            let exp_start = self.pos;
            self.advance();
            if self.peek() == Some('+') || self.peek() == Some('-') {
                self.advance();
            }
            let mut has_exponent_digits = false;
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() || ch == '_' {
                    self.advance();
                    has_exponent_digits = true;
                } else {
                    break;
                }
            }
            if !has_exponent_digits {
                let exp_end = self.pos;
                let exp_text = &self.source[exp_start..exp_end];
                self.diagnostics.push(GlyimDiagnostic::lex_error(
                    self.span(exp_start, exp_end),
                    format!("incomplete float exponent: '{}'", exp_text),
                ));
                self.pos = exp_start;
                return if is_float {
                    SyntaxKind::FloatLit
                } else {
                    SyntaxKind::IntLit
                };
            } else {
                is_float = true;
            }
        }

        self.lex_number_suffix();

        if is_float {
            SyntaxKind::FloatLit
        } else {
            SyntaxKind::IntLit
        }
    }

    fn lex_number_suffix(&mut self) {
        let suffix_start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
        if self.pos > suffix_start {
            let suffix = &self.source[suffix_start..self.pos];
            if !is_valid_number_suffix(suffix) {
                self.diagnostics.push(GlyimDiagnostic::lex_error(
                    Span::new(
                        self.file_id,
                        ByteIdx::from_raw(suffix_start as u32),
                        ByteIdx::from_raw(self.pos as u32),
                        SyntaxContext::ROOT,
                    ),
                    format!("invalid number suffix: '{}'", suffix),
                ));
            }
        }
    }

    fn lex_string(&mut self) {
        self.advance();
        while let Some(ch) = self.peek() {
            match ch {
                '"' => {
                    self.advance();
                    break;
                }
                '\\' => {
                    self.advance();
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn lex_char(&mut self) {
        self.advance();
        while let Some(ch) = self.peek() {
            match ch {
                '\'' => {
                    self.advance();
                    break;
                }
                '\\' => {
                    self.advance();
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn lex_dot(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some('.') {
            self.advance();
            if self.peek() == Some('=') {
                self.advance();
                SyntaxKind::DotDotEq
            } else {
                SyntaxKind::DotDot
            }
        } else {
            SyntaxKind::Dot
        }
    }

    fn lex_colon(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some(':') {
            self.advance();
            SyntaxKind::ColonColon
        } else {
            SyntaxKind::Colon
        }
    }

    fn lex_plus(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            SyntaxKind::PlusEq
        } else {
            SyntaxKind::Plus
        }
    }

    fn lex_minus(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some('>') {
            self.advance();
            SyntaxKind::Arrow
        } else if self.peek() == Some('=') {
            self.advance();
            SyntaxKind::MinusEq
        } else {
            SyntaxKind::Minus
        }
    }

    fn lex_star(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            SyntaxKind::StarEq
        } else {
            SyntaxKind::Star
        }
    }

    fn lex_slash(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            SyntaxKind::SlashEq
        } else {
            SyntaxKind::Slash
        }
    }

    fn lex_eq(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            SyntaxKind::EqEq
        } else if self.peek() == Some('>') {
            self.advance();
            SyntaxKind::FatArrow
        } else {
            SyntaxKind::Eq
        }
    }

    fn lex_bang(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            SyntaxKind::BangEq
        } else {
            SyntaxKind::Bang
        }
    }

    fn lex_lt(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            SyntaxKind::LtEq
        } else if self.peek() == Some('<') {
            self.advance();
            SyntaxKind::Shl
        } else {
            SyntaxKind::Lt
        }
    }

    fn lex_gt(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            SyntaxKind::GtEq
        } else if self.peek() == Some('>') {
            self.advance();
            SyntaxKind::Shr
        } else {
            SyntaxKind::Gt
        }
    }

    fn lex_and(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some('&') {
            self.advance();
            SyntaxKind::AndAnd
        } else {
            SyntaxKind::And
        }
    }

    fn lex_or(&mut self) -> SyntaxKind {
        self.advance();
        if self.peek() == Some('|') {
            self.advance();
            SyntaxKind::OrOr
        } else {
            SyntaxKind::Or
        }
    }
}

fn is_valid_number_suffix(suffix: &str) -> bool {
    matches!(
        suffix,
        "i8" | "i16"
            | "i32"
            | "i64"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
            | "f32"
            | "f64"
    )
}

fn lookup_keyword(ident: &str) -> SyntaxKind {
    match ident {
        "fn" => SyntaxKind::KwFn,
        "let" => SyntaxKind::KwLet,
        "struct" => SyntaxKind::KwStruct,
        "enum" => SyntaxKind::KwEnum,
        "if" => SyntaxKind::KwIf,
        "else" => SyntaxKind::KwElse,
        "return" => SyntaxKind::KwReturn,
        "match" => SyntaxKind::KwMatch,
        "mod" => SyntaxKind::KwMod,
        "comptime" => SyntaxKind::KwComptime,
        "self" => SyntaxKind::KwSelf,
        "super" => SyntaxKind::KwSuper,
        "crate" => SyntaxKind::KwCrate,
        "true" => SyntaxKind::KwTrue,
        "false" => SyntaxKind::KwFalse,
        "mut" => SyntaxKind::KwMut,
        "move" => SyntaxKind::KwMove,
        "ref" => SyntaxKind::KwRef,
        "as" => SyntaxKind::KwAs,
        "while" => SyntaxKind::KwWhile,
        "for" => SyntaxKind::KwFor,
        "in" => SyntaxKind::KwIn,
        "break" => SyntaxKind::KwBreak,
        "continue" => SyntaxKind::KwContinue,
        "loop" => SyntaxKind::KwLoop,
        "trait" => SyntaxKind::KwTrait,
        "impl" => SyntaxKind::KwImpl,
        "where" => SyntaxKind::KwWhere,
        "dyn" => SyntaxKind::KwDyn,
        "type" => SyntaxKind::KwType,
        "use" => SyntaxKind::KwUse,
        "pub" => SyntaxKind::KwPub,
        "priv" => SyntaxKind::KwPriv,
        "extern" => SyntaxKind::KwExtern,
        "unsafe" => SyntaxKind::KwUnsafe,
        "const" => SyntaxKind::KwConst,
        "static" => SyntaxKind::KwStatic,
        "macro_rules" => SyntaxKind::KwMacroRules,
        "_" => SyntaxKind::Underscore,
        _ => SyntaxKind::Ident,
    }
}

pub fn lex(source: &str, file_id: FileId) -> LexResult {
    Lexer::new(source, file_id).lex()
}

#[cfg(test)]
mod suffix_tests {
    use super::*;
    use glyim_span::FileId;

    #[test]
    fn test_invalid_number_suffix_produces_error() {
        let result = Lexer::new("42abc", FileId::from_raw(0)).lex();
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("suffix") || d.message.contains("invalid")),
            "Expected error for invalid suffix 'abc', got: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_valid_i32_suffix() {
        let result = Lexer::new("42i32", FileId::from_raw(0)).lex();
        assert!(
            result.diagnostics.is_empty(),
            "i32 suffix should be valid: {:?}",
            result.diagnostics
        );
        assert!(
            result
                .tokens
                .iter()
                .any(|t| t.kind == SyntaxKind::IntLit && t.text == "42i32")
        );
    }

    #[test]
    fn test_valid_u8_suffix() {
        let result = Lexer::new("255u8", FileId::from_raw(0)).lex();
        assert!(
            result.diagnostics.is_empty(),
            "u8 suffix should be valid: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_valid_f64_suffix() {
        let result = Lexer::new("3.14f64", FileId::from_raw(0)).lex();
        assert!(
            result.diagnostics.is_empty(),
            "f64 suffix should be valid: {:?}",
            result.diagnostics
        );
    }
}
