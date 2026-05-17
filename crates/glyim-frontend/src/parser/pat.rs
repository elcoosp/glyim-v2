use super::Parser;
use glyim_syntax::SyntaxKind;

impl<'a> Parser<'a> {
    pub(crate) fn parse_pat(&mut self) {
        let cp = self.checkpoint();
        self.parse_pat_single();
        if self.current_kind() == SyntaxKind::Or {
            self.start_node_at(cp, SyntaxKind::PatOr);
            while self.current_kind() == SyntaxKind::Or {
                self.bump(); // |
                self.parse_pat_single();
            }
            self.finish_node(); // PatOr
        }
    }

    pub(crate) fn parse_pat_single(&mut self) {
        match self.current_kind() {
            SyntaxKind::KwRef => {
                self.bump(); // ref
                if self.current_kind() == SyntaxKind::KwMut {
                    self.bump(); // mut
                }
                self.parse_pat_inner();
            }
            SyntaxKind::KwMut => {
                self.bump(); // mut
                self.parse_pat_inner();
            }
            SyntaxKind::AndAnd => {
                self.skip_token(); // skip &&
                self.parse_pat_inner();
            }
            SyntaxKind::And => {
                self.bump(); // &
                if self.current_kind() == SyntaxKind::KwMut {
                    self.bump(); // mut
                }
                self.parse_pat_inner();
            }
            SyntaxKind::LParen => {
                self.start_node(SyntaxKind::PatTuple);
                self.bump(); // (
                while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
                    self.parse_pat();
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::RParen);
                self.finish_node();
            }
            _ => {
                self.parse_pat_inner();
            }
        }
    }

    pub(crate) fn parse_pat_inner(&mut self) {
        match self.current_kind() {
            SyntaxKind::Bang => {
                self.start_node(SyntaxKind::NeverType);
                self.bump(); // !
                self.finish_node();
            }
            SyntaxKind::Underscore => {
                self.start_node(SyntaxKind::PatWild);
                self.bump();
                self.finish_node();
            }
            SyntaxKind::Ident | SyntaxKind::KwSelf | SyntaxKind::KwSuper | SyntaxKind::KwCrate => {
                let next = self.peek_kind().unwrap_or(SyntaxKind::Error);
                if next == SyntaxKind::ColonColon
                    || next == SyntaxKind::LParen
                    || next == SyntaxKind::LBrace
                {
                    self.start_node(SyntaxKind::UsePath);
                    self.parse_path_inner();
                    self.finish_node();
                } else {
                    self.start_node(SyntaxKind::PatIdent);
                    self.bump();
                    self.finish_node();
                    return;
                }
                if self.current_kind() == SyntaxKind::LParen {
                    self.start_node(SyntaxKind::PatTuple);
                    self.bump(); // (
                    while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
                        self.parse_pat();
                        if self.current_kind() == SyntaxKind::Comma {
                            self.bump();
                        }
                    }
                    self.expect(SyntaxKind::RParen);
                    self.finish_node();
                } else if self.current_kind() == SyntaxKind::LBrace {
                    self.start_node(SyntaxKind::PatStruct);
                    self.bump(); // {
                    while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                        if self.current_kind() == SyntaxKind::DotDot {
                            self.bump(); // ..
                            // trailing comma allowed after ..
                            if self.current_kind() == SyntaxKind::Comma {
                                self.bump();
                            }
                        } else if self.current_kind() == SyntaxKind::Ident {
                            let cp = self.checkpoint();
                            self.bump(); // field name
                            if self.current_kind() == SyntaxKind::Colon {
                                // explicit: field_name: pattern
                                self.start_node_at(cp, SyntaxKind::PatIdent);
                                self.finish_node();
                                self.bump(); // :
                                self.parse_pat();
                            } else {
                                // shorthand: field_name (binding)
                                self.start_node_at(cp, SyntaxKind::PatIdent);
                                self.finish_node();
                            }
                        } else {
                            self.error("expected field pattern");
                            if self.current().is_some() {
                                self.bump();
                            }
                        }
                        if self.current_kind() == SyntaxKind::Comma {
                            self.bump();
                        }
                    }
                    self.expect(SyntaxKind::RBrace);
                    self.finish_node();
                }
            }
            SyntaxKind::IntLit
            | SyntaxKind::FloatLit
            | SyntaxKind::StringLit
            | SyntaxKind::CharLit
            | SyntaxKind::KwTrue
            | SyntaxKind::KwFalse => {
                self.start_node(SyntaxKind::PatLit);
                self.bump();
                // Check for range pattern: 0..10 or 0..=10
                if matches!(
                    self.current_kind(),
                    SyntaxKind::DotDot | SyntaxKind::DotDotEq
                ) {
                    self.bump(); // .. or ..=
                    // Parse the end pattern (could be literal, path, or wildcard)
                    if !matches!(
                        self.current_kind(),
                        SyntaxKind::FatArrow | SyntaxKind::Comma | SyntaxKind::RBrace
                    ) {
                        self.parse_pat();
                    }
                }
                self.finish_node();
            }
            _ => {
                self.error(format!("expected pattern, found {:?}", self.current_kind()));
                if self.current().is_some() {
                    self.bump();
                }
            }
        }
    }
}
