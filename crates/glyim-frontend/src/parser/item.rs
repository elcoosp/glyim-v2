use super::Parser;
use glyim_syntax::{GlyimLang, SyntaxKind};
use rowan::Language; // Required for kind_to_raw

impl<'a> Parser<'a> {

    pub(crate) fn parse_item(&mut self) {
        let _vis = self.parse_visibility();

        if self.current_kind() == SyntaxKind::KwUnsafe {
            self.bump(); // unsafe
            if !matches!(
                self.current_kind(),
                SyntaxKind::KwFn
                    | SyntaxKind::KwStruct
                    | SyntaxKind::KwUse
                    | SyntaxKind::KwEnum
                    | SyntaxKind::KwTrait
                    | SyntaxKind::KwImpl
                    | SyntaxKind::KwMod
                    | SyntaxKind::KwConst
                    | SyntaxKind::KwStatic
                    | SyntaxKind::KwType
                    | SyntaxKind::KwExtern
            ) {
                self.error("expected item after 'unsafe'");
                return;
            }
        }

        match self.current_kind() {
            SyntaxKind::KwFn => self.parse_fn_def(),
            SyntaxKind::KwStruct => self.parse_struct_def(),
            SyntaxKind::KwUse => self.parse_use_decl(),
            SyntaxKind::KwEnum => self.parse_enum_def(),
            SyntaxKind::KwTrait => self.parse_trait_def(),
            SyntaxKind::KwImpl => self.parse_impl_def(),
            SyntaxKind::KwMod => {
                self.start_node(SyntaxKind::Module);
                self.bump(); // mod
                self.bump_expected(SyntaxKind::Ident);
                if self.current_kind() == SyntaxKind::LBrace {
                    self.bump(); // {
                    while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                        self.parse_item();
                    }
                    self.expect(SyntaxKind::RBrace);
                } else {
                    self.expect(SyntaxKind::Semicolon);
                }
                self.finish_node(); // Module
            }
            SyntaxKind::KwConst => {
                self.start_node(SyntaxKind::ConstDef);
                self.bump(); // const
                self.bump_expected(SyntaxKind::Ident);
                self.expect(SyntaxKind::Colon);
                self.parse_type();
                if self.current_kind() == SyntaxKind::Eq {
                    self.bump();
                    self.parse_expr();
                }
                self.expect(SyntaxKind::Semicolon);
                self.finish_node();
            }
            SyntaxKind::KwStatic => {
                self.start_node(SyntaxKind::StaticDef);
                self.bump(); // static
                if self.current_kind() == SyntaxKind::KwRef {
                    self.bump();
                }
                if self.current_kind() == SyntaxKind::KwMut {
                    self.bump();
                }
                self.bump_expected(SyntaxKind::Ident);
                self.expect(SyntaxKind::Colon);
                self.parse_type();
                if self.current_kind() == SyntaxKind::Eq {
                    self.bump();
                    self.parse_expr();
                }
                self.expect(SyntaxKind::Semicolon);
                self.finish_node();
            }
            SyntaxKind::KwType => {
                self.start_node(SyntaxKind::TypeAlias);
                self.bump(); // type
                self.bump_expected(SyntaxKind::Ident);
                if self.current_kind() == SyntaxKind::Lt {
                    self.parse_type_param_list();
                }
                if self.current_kind() == SyntaxKind::Eq {
                    self.bump();
                    self.parse_type();
                }
                self.expect(SyntaxKind::Semicolon);
                self.finish_node();
            }
            SyntaxKind::KwMacroRules => {
                self.parse_macro_def();
            }
            SyntaxKind::KwExtern => {
                self.start_node(SyntaxKind::ExternBlock);
                self.bump(); // extern
                if self.current_kind() == SyntaxKind::StringLit {
                    self.bump(); // ABI string
                }
                if self.current_kind() == SyntaxKind::LBrace {
                    self.parse_block_inner();
                } else {
                    self.expect(SyntaxKind::Semicolon);
                }
                self.finish_node();
            }
            _ => {
                self.error(format!("expected item, found {:?}", self.current_kind()));
                while self.current().is_some()
                    && !matches!(
                        self.current_kind(),
                        SyntaxKind::KwFn
                            | SyntaxKind::KwStruct
                            | SyntaxKind::KwUse
                            | SyntaxKind::KwEnum
                            | SyntaxKind::KwTrait
                            | SyntaxKind::KwImpl
                            | SyntaxKind::KwMod
                            | SyntaxKind::KwConst
                            | SyntaxKind::KwStatic
                            | SyntaxKind::KwType
                            | SyntaxKind::KwExtern
                            | SyntaxKind::KwPub
                            | SyntaxKind::KwUnsafe
                            | SyntaxKind::KwMacroRules
                    )
                {
                    self.bump();
                }
            }
        }
    }

    pub(crate) fn parse_visibility(&mut self) -> bool {
        if self.current_kind() == SyntaxKind::KwPub {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn parse_macro_def(&mut self) {
        self.start_node(SyntaxKind::MacroDef);
        self.bump_expected(SyntaxKind::KwMacroRules);
        self.bump(); // !
        self.bump_expected(SyntaxKind::Ident); // macro name
        // Body: { arms }
        self.expect(SyntaxKind::LBrace);
        while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
            // Skip separators between arms
            if matches!(
                self.current_kind(),
                SyntaxKind::Comma | SyntaxKind::Semicolon
            ) {
                self.bump();
                continue;
            }
            self.start_node(SyntaxKind::MacroArm);
            // Pattern: token tree (must be parenthesized token tree)
            self.parse_token_tree();
            self.expect(SyntaxKind::FatArrow);
            // Expansion: token tree (must be braced token tree)
            self.parse_token_tree();
            self.finish_node(); // MacroArm
            // Consume optional separator
            if matches!(
                self.current_kind(),
                SyntaxKind::Comma | SyntaxKind::Semicolon
            ) {
                self.bump();
            }
        }
        self.expect(SyntaxKind::RBrace);
        self.finish_node(); // MacroDef
    }

    pub(crate) fn parse_token_tree(&mut self) {
        // Parse a balanced token tree: either a single token, or a group delimited by parens/braces/brackets
        match self.current_kind() {
            SyntaxKind::LParen => {
                self.start_node(SyntaxKind::TokenTree);
                self.bump(); // (
                while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
                    // Match repetition pattern: $(...)sep?rep_op or metavar: $name:fragment
                    if self.current_kind() == SyntaxKind::Dollar {
                        self.start_node(SyntaxKind::TokenTree);
                        self.bump(); // $
                        if self.current_kind() == SyntaxKind::LParen {
                            // Repetition: $(...)...
                            self.parse_token_tree(); // parse the parenthesized group
                            // Optional separator
                            if matches!(
                                self.current_kind(),
                                SyntaxKind::Comma | SyntaxKind::Semicolon
                            ) {
                                self.bump();
                            }
                            // Repetition operator
                            if matches!(
                                self.current_kind(),
                                SyntaxKind::Plus | SyntaxKind::Star | SyntaxKind::Question
                            ) {
                                self.bump();
                            }
                        } else if self.current_kind() == SyntaxKind::Ident {
                            // Metavariable: $name or $name:fragment
                            self.bump(); // variable name
                            if self.current_kind() == SyntaxKind::Colon {
                                self.bump(); // :
                                // Parse fragment specifier
                                self.bump();
                            }
                        } else {
                            // Standalone $ token
                            self.bump();
                        }
                        self.finish_node(); // TokenTree for dollar construct
                    } else {
                        // Regular token tree item
                        if matches!(
                            self.current_kind(),
                            SyntaxKind::LParen | SyntaxKind::LBrace | SyntaxKind::LBracket
                        ) {
                            self.parse_token_tree();
                        } else {
                            self.start_node(SyntaxKind::TokenTree);
                            self.bump();
                            self.finish_node();
                        }
                    }
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::RParen);
                self.finish_node(); // TokenTree group
            }
            SyntaxKind::LBrace => {
                self.start_node(SyntaxKind::TokenTree);
                self.bump();
                while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                    if matches!(
                        self.current_kind(),
                        SyntaxKind::LParen | SyntaxKind::LBrace | SyntaxKind::LBracket
                    ) {
                        self.parse_token_tree();
                    } else {
                        self.start_node(SyntaxKind::TokenTree);
                        self.bump();
                        self.finish_node();
                    }
                }
                self.expect(SyntaxKind::RBrace);
                self.finish_node();
            }
            SyntaxKind::LBracket => {
                self.start_node(SyntaxKind::TokenTree);
                self.bump();
                while self.current_kind() != SyntaxKind::RBracket && self.current().is_some() {
                    if matches!(
                        self.current_kind(),
                        SyntaxKind::LParen | SyntaxKind::LBrace | SyntaxKind::LBracket
                    ) {
                        self.parse_token_tree();
                    } else {
                        self.start_node(SyntaxKind::TokenTree);
                        self.bump();
                        self.finish_node();
                    }
                }
                self.expect(SyntaxKind::RBracket);
                self.finish_node();
            }
            _ => {
                // Single token
                self.start_node(SyntaxKind::TokenTree);
                self.bump();
                self.finish_node();
            }
        }
    }

    pub(crate) fn parse_fn_def(&mut self) {
        self.start_node(SyntaxKind::FnDef);
        self.bump_expected(SyntaxKind::KwFn);
        self.bump_expected(SyntaxKind::Ident);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        self.expect(SyntaxKind::LParen);
        self.start_node(SyntaxKind::ParamList);
        while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
            self.parse_param();
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        self.finish_node(); // ParamList
        self.expect(SyntaxKind::RParen);
        if self.current_kind() == SyntaxKind::Arrow {
            self.bump();
            self.parse_type();
        }
        if self.current_kind() == SyntaxKind::LBrace {
            self.parse_block();
        } else {
            self.expect(SyntaxKind::Semicolon);
        }
        self.finish_node(); // FnDef
    }

    pub(crate) fn parse_param(&mut self) {
        self.start_node(SyntaxKind::Param);
        if self.current_kind() == SyntaxKind::And {
            self.bump(); // &
            if self.current_kind() == SyntaxKind::KwMut {
                self.bump(); // mut
            }
            if self.current_kind() == SyntaxKind::KwSelf {
                self.bump(); // self
                self.finish_node();
                return;
            }
            self.bump_expected(SyntaxKind::Ident);
            self.expect(SyntaxKind::Colon);
            self.parse_type();
        } else if self.current_kind() == SyntaxKind::KwMut {
            self.bump(); // mut
            if self.current_kind() == SyntaxKind::KwSelf {
                self.bump(); // self
                self.finish_node();
                return;
            }
            self.bump_expected(SyntaxKind::Ident);
            self.expect(SyntaxKind::Colon);
            self.parse_type();
        } else if self.current_kind() == SyntaxKind::KwSelf {
            self.bump(); // self
            self.finish_node();
            return;
        } else {
            self.bump_expected(SyntaxKind::Ident);
            self.expect(SyntaxKind::Colon);
            self.parse_type();
        }
        self.finish_node(); // Param
    }

    pub(crate) fn parse_struct_def(&mut self) {
        self.start_node(SyntaxKind::StructDef);
        self.bump_expected(SyntaxKind::KwStruct);
        self.bump_expected(SyntaxKind::Ident);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        match self.current_kind() {
            SyntaxKind::LParen => {
                self.bump(); // (
                while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::RParen);
                self.expect(SyntaxKind::Semicolon);
            }
            SyntaxKind::LBrace => {
                self.bump(); // {
                while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                    if self.current_kind() == SyntaxKind::Ident {
                        self.bump();
                        if self.current_kind() == SyntaxKind::Colon {
                            self.bump();
                            self.parse_type();
                        }
                    } else {
                        self.error("expected field name");
                        self.bump();
                    }
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::RBrace);
            }
            _ => {
                self.expect(SyntaxKind::Semicolon);
            }
        }
        self.finish_node(); // StructDef
    }

    pub(crate) fn parse_enum_def(&mut self) {
        self.start_node(SyntaxKind::EnumDef);
        self.bump_expected(SyntaxKind::KwEnum);
        self.bump_expected(SyntaxKind::Ident);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        self.expect(SyntaxKind::LBrace);
        self.start_node(SyntaxKind::VariantList);
        while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
            self.start_node(SyntaxKind::EnumVariant);
            self.bump_expected(SyntaxKind::Ident);
            if self.current_kind() == SyntaxKind::LParen {
                self.bump(); // (
                while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::RParen);
            }
            if self.current_kind() == SyntaxKind::LBrace {
                self.start_node(SyntaxKind::FieldList);
                self.bump(); // {
                while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                    self.start_node(SyntaxKind::StructField);
                    if self.current_kind() == SyntaxKind::Ident {
                        self.bump(); // field name
                        if self.current_kind() == SyntaxKind::Colon {
                            self.bump(); // :
                            self.parse_type();
                        }
                    } else {
                        self.error("expected field name");
                        if self.current().is_some() {
                            self.bump();
                        }
                    }
                    self.finish_node(); // StructField
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.finish_node(); // FieldList
                self.expect(SyntaxKind::RBrace);
            }
            if self.current_kind() == SyntaxKind::Eq {
                self.bump();
                self.parse_expr();
            }
            self.finish_node(); // EnumVariant
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        self.finish_node(); // VariantList
        self.expect(SyntaxKind::RBrace);
        self.finish_node(); // EnumDef
    }

    pub(crate) fn parse_trait_def(&mut self) {
        self.start_node(SyntaxKind::TraitDef);
        self.bump_expected(SyntaxKind::KwTrait);
        self.bump_expected(SyntaxKind::Ident);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        if self.current_kind() == SyntaxKind::Colon {
            self.bump(); // :
            loop {
                self.parse_type();
                if self.current_kind() == SyntaxKind::Plus {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        if self.current_kind() == SyntaxKind::KwWhere {
            self.parse_where_clause();
        }
        self.expect(SyntaxKind::LBrace);
        while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
            match self.current_kind() {
                SyntaxKind::KwFn => self.parse_fn_def(),
                SyntaxKind::KwType => {
                    self.start_node(SyntaxKind::TypeAlias);
                    self.bump(); // type
                    self.bump_expected(SyntaxKind::Ident);
                    if self.current_kind() == SyntaxKind::Colon {
                        self.bump();
                        loop {
                            self.parse_type();
                            if self.current_kind() == SyntaxKind::Plus {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(SyntaxKind::Semicolon);
                    self.finish_node();
                }
                SyntaxKind::KwConst => {
                    self.start_node(SyntaxKind::ConstDef);
                    self.bump(); // const
                    self.bump_expected(SyntaxKind::Ident);
                    self.expect(SyntaxKind::Colon);
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Eq {
                        self.bump();
                        self.parse_expr();
                    }
                    self.expect(SyntaxKind::Semicolon);
                    self.finish_node();
                }
                _ => {
                    self.error(format!(
                        "expected trait item, found {:?}",
                        self.current_kind()
                    ));
                    self.bump();
                }
            }
        }
        self.expect(SyntaxKind::RBrace);
        self.finish_node(); // TraitDef
    }

    pub(crate) fn parse_impl_def(&mut self) {
        self.start_node(SyntaxKind::ImplDef);
        self.bump_expected(SyntaxKind::KwImpl);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        self.parse_type();
        if self.current_kind() == SyntaxKind::KwFor {
            self.bump(); // for
            self.parse_type();
        }
        if self.current_kind() == SyntaxKind::KwWhere {
            self.parse_where_clause();
        }
        self.expect(SyntaxKind::LBrace);
        while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
            match self.current_kind() {
                SyntaxKind::KwFn => self.parse_fn_def(),
                SyntaxKind::KwType => {
                    self.start_node(SyntaxKind::TypeAlias);
                    self.bump(); // type
                    self.bump_expected(SyntaxKind::Ident);
                    if self.current_kind() == SyntaxKind::Eq {
                        self.bump();
                        self.parse_type();
                    }
                    self.expect(SyntaxKind::Semicolon);
                    self.finish_node();
                }
                SyntaxKind::KwConst => {
                    self.start_node(SyntaxKind::ConstDef);
                    self.bump(); // const
                    self.bump_expected(SyntaxKind::Ident);
                    self.expect(SyntaxKind::Colon);
                    self.parse_type();
                    self.expect(SyntaxKind::Eq);
                    self.parse_expr();
                    self.expect(SyntaxKind::Semicolon);
                    self.finish_node();
                }
                _ => {
                    self.error(format!(
                        "expected impl item, found {:?}",
                        self.current_kind()
                    ));
                    self.bump();
                }
            }
        }
        self.expect(SyntaxKind::RBrace);
        self.finish_node(); // ImplDef
    }

    pub(crate) fn parse_where_clause(&mut self) {
        self.start_node(SyntaxKind::WhereClause);
        self.expect(SyntaxKind::KwWhere);
        while self.current_kind() != SyntaxKind::LBrace
            && self.current_kind() != SyntaxKind::Semicolon
            && self.current().is_some()
        {
            self.parse_type();
            if self.current_kind() == SyntaxKind::Colon {
                self.bump(); // :
                loop {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Lt {
                        self.parse_type_param_list();
                    }
                    if self.current_kind() == SyntaxKind::Plus {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            } else if self.current_kind() != SyntaxKind::LBrace
                && self.current_kind() != SyntaxKind::Semicolon
            {
                self.error("expected ',' or end of where clause");
                break;
            }
        }
        self.finish_node();
    }

    pub(crate) fn parse_type_param_list(&mut self) {
        self.start_node(SyntaxKind::TypeParamList);
        self.expect(SyntaxKind::Lt);
        while self.current_kind() != SyntaxKind::Gt
            && self.current_kind() != SyntaxKind::Shr
            && self.current().is_some()
        {
            self.start_node(SyntaxKind::TypeParam);
            self.bump_expected(SyntaxKind::Ident);
            if self.current_kind() == SyntaxKind::Colon {
                self.bump();
                loop {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Plus {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            if self.current_kind() == SyntaxKind::Eq {
                self.bump();
                self.parse_type();
            }
            self.finish_node(); // TypeParam
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        // Handle >> as two Gt tokens: emit both, one for this list, one queued
        if self.current_kind() == SyntaxKind::Shr {
            self.builder
                .token(GlyimLang::kind_to_raw(SyntaxKind::Gt), ">");
            self.builder
                .token(GlyimLang::kind_to_raw(SyntaxKind::Gt), ">");
            self.pos += 1; // skip the Shr token
            self.pending_gt_count += 1;
            // Do NOT consume from pending here.
            self.finish_node();
            return;
        }
        // Not Shr - consume a real Gt token
        if self.current_kind() == SyntaxKind::Gt {
            self.bump();
        }
        self.finish_node();
    }

    pub(crate) fn parse_type_arg_list(&mut self) {
        self.bump(); // <
        while self.current_kind() != SyntaxKind::Gt
            && self.current_kind() != SyntaxKind::Shr
            && self.current().is_some()
        {
            self.parse_type();
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        // Handle >> as two Gt tokens:
        // Emit both now. The first closes this list, the second is queued for the outer list.
        if self.current_kind() == SyntaxKind::Shr {
            // Emit Gt for THIS list (closes this type arg list)
            self.builder
                .token(GlyimLang::kind_to_raw(SyntaxKind::Gt), ">");
            // Emit Gt for the OUTER list (queued via pending_gt_count)
            self.builder
                .token(GlyimLang::kind_to_raw(SyntaxKind::Gt), ">");
            self.pos += 1; // skip the Shr token
            self.pending_gt_count += 1; // outer list will see Gt via current_kind
            // Do NOT consume from pending here - the Gt for this list was already emitted.
            return;
        }
        // Not Shr - consume a real Gt token
        if self.current_kind() == SyntaxKind::Gt {
            self.bump();
        }
    }

    pub(crate) fn parse_use_decl(&mut self) {
        self.start_node(SyntaxKind::UseDecl);
        self.bump_expected(SyntaxKind::KwUse);
        self.parse_use_tree();
        self.expect(SyntaxKind::Semicolon);
        self.finish_node();
    }

    pub(crate) fn parse_use_tree(&mut self) {
        self.start_node(SyntaxKind::UseTree);

        // Parse the path (could be simple or qualified)
        self.parse_use_path();

        // Handle glob import `*`
        if self.current_kind() == SyntaxKind::Star {
            self.bump(); // *
        }

        // Handle nested imports `{a, b, c}`
        if self.current_kind() == SyntaxKind::LBrace {
            self.bump(); // {
            while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                self.parse_use_tree();
                if self.current_kind() == SyntaxKind::Comma {
                    self.bump();
                }
            }
            self.expect(SyntaxKind::RBrace);
        }

        self.finish_node();
    }

    pub(crate) fn parse_use_path(&mut self) {
        // For use declarations, we need to parse the full path including :: segments
        self.start_node(SyntaxKind::UsePath);

        // First segment
        match self.current_kind() {
            SyntaxKind::Ident | SyntaxKind::KwSelf | SyntaxKind::KwSuper | SyntaxKind::KwCrate => {
                self.bump();
            }
            _ => {
                self.error("expected identifier in use path");
                return;
            }
        }

        // Continue with :: segments
        while self.current_kind() == SyntaxKind::ColonColon {
            self.bump(); // ::
            // After :: we can have an identifier, * (glob), or { for nested imports
            if self.current_kind() == SyntaxKind::Star || self.current_kind() == SyntaxKind::LBrace
            {
                break;
            }
            match self.current_kind() {
                SyntaxKind::Ident
                | SyntaxKind::KwSelf
                | SyntaxKind::KwSuper
                | SyntaxKind::KwCrate => {
                    self.bump();
                }
                _ => {
                    self.error("expected identifier, '*', or '{{' after '::'");
                    break;
                }
            }
        }

        self.finish_node();
    }

}
