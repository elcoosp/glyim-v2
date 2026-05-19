mod matcher;
mod substitution;
mod token_tree;

use crate::BuiltinMacro;
use glyim_core::interner::{Interner, Name};
use glyim_diag::GlyimDiagnostic;
use glyim_span::{
    ByteIdx, ExpnData, ExpnKind, FileId, HygieneCtx, Mark, Span, SyntaxContext, Transparency,
};
use glyim_syntax::{GlyimLang, GreenNode, SyntaxKind, SyntaxNode};
use rowan::Language;
use smol_str::SmolStr;
use std::collections::HashMap;

use matcher::{MatchResult, Pattern, match_pattern};
use token_tree::{TokenTree, flatten_token_tree};

const MAX_RECURSION_DEPTH: u32 = 128;

#[derive(Clone, Debug)]
pub(crate) struct MacroArm {
    pattern: Pattern,
    expansion: Vec<TokenTree>,
}

#[derive(Clone, Debug)]
pub(crate) struct MacroDef {
    pub(crate) name: Name,
    arms: Vec<MacroArm>,
}

pub(crate) fn expand_crate(
    root: &SyntaxNode,
    interner: &mut Interner,
    hygiene: &mut HygieneCtx,
    registered: &[crate::MacroDef],
) -> (GreenNode, Vec<GlyimDiagnostic>) {
    let mut expander = ExpanderImpl::new(hygiene, interner.clone());
    // Register builtins from the public API
    for def in registered {
        if let crate::MacroKind::Builtin { handler, .. } = &def.kind {
            expander.registered_builtins.insert(def.name, *handler);
        }
    }
    expander.collect_macros(root, interner);
    let (green, diags) = expander.expand_node(root, 0);
    (green, diags)
}

pub(crate) fn expand_macro_invocation(
    name: Name,
    args: &SyntaxNode,
    call_site: Span,
    hygiene: &mut HygieneCtx,
    registered: &[crate::MacroDef],
    interner: &Interner,
    depth: u32,
) -> (Option<GreenNode>, Vec<GlyimDiagnostic>) {
    let mut registered_builtins: HashMap<Name, BuiltinMacro> = HashMap::new();
    for def in registered {
        if let crate::MacroKind::Builtin { handler, .. } = &def.kind {
            registered_builtins.insert(def.name, *handler);
        }
    }

    // Check registered builtins first
    if let Some(handler) = registered_builtins.get(&name).copied() {
        let mut expander = ExpanderImpl {
            hygiene,
            macros: HashMap::new(),
            registered_builtins,
            diagnostics: Vec::new(),
            interner: interner.clone(),
        };
        return expander.expand_builtin(handler, args, call_site, depth);
    }

    let mut expander = ExpanderImpl {
        hygiene,
        macros: HashMap::new(),
        registered_builtins,
        diagnostics: Vec::new(),
        interner: interner.clone(),
    };
    let (green, diags) = expander.expand_macro_call(name, args, call_site, depth);
    expander.diagnostics.extend(diags);
    (green, expander.diagnostics)
}

pub(crate) struct ExpanderImpl<'a> {
    hygiene: &'a mut HygieneCtx,
    macros: HashMap<Name, MacroDef>,
    registered_builtins: HashMap<Name, BuiltinMacro>,
    diagnostics: Vec<GlyimDiagnostic>,
    interner: Interner,
}

impl<'a> ExpanderImpl<'a> {
    pub(crate) fn new(hygiene: &'a mut HygieneCtx, interner: Interner) -> Self {
        Self {
            hygiene,
            macros: HashMap::new(),
            registered_builtins: HashMap::new(),
            diagnostics: Vec::new(),
            interner,
        }
    }

    pub(crate) fn collect_macros(&mut self, node: &SyntaxNode, _interner: &mut Interner) {
        for child in node.children() {
            if child.kind() == SyntaxKind::MacroDef {
                if let Some(def) = self.parse_macro_def(&child) {
                    self.macros.insert(def.name, def);
                }
            } else {
                self.collect_macros(&child, _interner);
            }
        }
    }

    fn parse_macro_def(&mut self, node: &SyntaxNode) -> Option<MacroDef> {
        let mut ident_text = None;
        for child in node.children_with_tokens() {
            if child.kind() == SyntaxKind::Ident {
                ident_text = child.into_token().map(|t| t.text().to_string());
                break;
            }
        }
        let name_str = ident_text?;
        let name = self.interner.intern(&name_str);
        let mut arms = Vec::new();
        for arm_node in node.children().filter(|n| n.kind() == SyntaxKind::MacroArm) {
            if let Some(arm) = self.parse_macro_arm(&arm_node) {
                arms.push(arm);
            }
        }
        Some(MacroDef { name, arms })
    }

    fn parse_macro_arm(&self, node: &SyntaxNode) -> Option<MacroArm> {
        let mut children = node.children();
        let pattern_node = children.find(|c| c.kind() == SyntaxKind::TokenTree)?;
        let pattern = self.parse_pattern(&pattern_node)?;
        let expansion_node = children.find(|c| c.kind() == SyntaxKind::TokenTree)?;
        let expansion = self.parse_expansion(&expansion_node);
        Some(MacroArm { pattern, expansion })
    }

    fn parse_pattern(&self, node: &SyntaxNode) -> Option<Pattern> {
        matcher::parse_pattern_from_node(node)
    }

    fn parse_expansion(&self, node: &SyntaxNode) -> Vec<TokenTree> {
        token_tree::collect_token_trees(node)
    }

    pub(crate) fn expand_node(
        &mut self,
        node: &SyntaxNode,
        depth: u32,
    ) -> (GreenNode, Vec<GlyimDiagnostic>) {
        use rowan::GreenNodeBuilder;
        let mut builder = GreenNodeBuilder::new();
        let mut diagnostics = Vec::new();

        self.expand_node_recursive(node, depth, &mut builder, &mut diagnostics);

        let green = builder.finish();
        (green, diagnostics)
    }

    fn expand_node_recursive(
        &mut self,
        node: &SyntaxNode,
        depth: u32,
        builder: &mut rowan::GreenNodeBuilder,
        diagnostics: &mut Vec<GlyimDiagnostic>,
    ) {
        if node.kind() == SyntaxKind::MacroCall {
            let (expanded_green, mut diags) = self.try_expand_macro_call(node, depth);
            diagnostics.append(&mut diags);
            if let Some(green) = expanded_green {
                // Re-parse the expanded token stream in a function body context
                // so that expression/statement tokens are correctly parsed as MacroCalls.
                let temp_root = SyntaxNode::new_root(green.clone());
                let token_text = temp_root.text().to_string();
                // Wrap in a function body to parse in statement context
                let wrapped = format!("fn __glyim_expanded() {{ {} }}", token_text);
                let parse_result = glyim_frontend::parse_to_syntax(&wrapped, FileId::BOGUS);
                let reparsed_root = parse_result.root;
                // Find the function body block and expand its statements
                for child in reparsed_root.children_with_tokens() {
                    match child {
                        rowan::NodeOrToken::Node(n) => {
                            if n.kind() == SyntaxKind::FnDef
                                && let Some(block) =
                                    n.children().find(|c| c.kind() == SyntaxKind::Block)
                            {
                                for stmt in block.children_with_tokens() {
                                    match stmt {
                                        rowan::NodeOrToken::Node(s) => {
                                            self.expand_node_recursive(
                                                &s,
                                                depth + 1,
                                                builder,
                                                diagnostics,
                                            );
                                        }
                                        rowan::NodeOrToken::Token(t) => {
                                            let kind = GlyimLang::kind_to_raw(t.kind());
                                            builder.token(kind, t.text());
                                        }
                                    }
                                }
                            }
                        }
                        rowan::NodeOrToken::Token(t) => {
                            let kind = GlyimLang::kind_to_raw(t.kind());
                            builder.token(kind, t.text());
                        }
                    }
                }
                return;
            }
        }

        if node.kind() == SyntaxKind::MacroDef {
            return;
        }

        // Copy other nodes recursively
        builder.start_node(GlyimLang::kind_to_raw(node.kind()));
        for child in node.children_with_tokens() {
            match child {
                rowan::NodeOrToken::Node(n) => {
                    self.expand_node_recursive(&n, depth, builder, diagnostics);
                }
                rowan::NodeOrToken::Token(t) => {
                    let kind = GlyimLang::kind_to_raw(t.kind());
                    builder.token(kind, t.text());
                }
            }
        }
        builder.finish_node();
    }

    /// Find the macro name in a MacroCall node.
    /// The macro name is the Ident token immediately before the `!` token.
    fn find_macro_name(node: &SyntaxNode) -> Option<String> {
        let mut last_ident: Option<String> = None;
        for child in node.children_with_tokens() {
            match &child {
                rowan::NodeOrToken::Token(t) => {
                    if t.kind() == SyntaxKind::Bang {
                        // Found `!` — return the ident we saw just before it
                        return last_ident;
                    }
                    if t.kind() == SyntaxKind::Ident {
                        last_ident = Some(t.text().to_string());
                    } else {
                        last_ident = None;
                    }
                }
                rowan::NodeOrToken::Node(n) => {
                    // Recurse into child nodes, but only use result if we
                    // haven't seen a `!` at this level
                    if let Some(ident) = Self::find_macro_name(n) {
                        return Some(ident);
                    }
                    last_ident = None;
                }
            }
        }
        // If no `!` found, fall back to the first ident we saw
        last_ident
    }

    fn try_expand_macro_call(
        &mut self,
        node: &SyntaxNode,
        depth: u32,
    ) -> (Option<GreenNode>, Vec<GlyimDiagnostic>) {
        if depth > MAX_RECURSION_DEPTH {
            return (
                None,
                vec![GlyimDiagnostic::type_error(
                    Span::DUMMY,
                    "macro recursion limit exceeded",
                )],
            );
        }

        // Find the macro name (the ident before the ! token)
        let ident_text = Self::find_macro_name(node);
        let name_token_text = match ident_text {
            Some(t) => t,
            None => return (None, Vec::new()),
        };

        let name = self.interner.intern(&name_token_text);
        let args_node = match node.children().find(|c| c.kind() == SyntaxKind::TokenTree) {
            Some(n) => n,
            None => return (None, Vec::new()),
        };

        let call_site = self.span_from_node(node);

        // Check registered builtins first
        if let Some(handler) = self.registered_builtins.get(&name).copied() {
            return self.expand_builtin(handler, &args_node, call_site, depth);
        }

        self.expand_macro_call(name, &args_node, call_site, depth)
    }

    fn expand_macro_call(
        &mut self,
        name: Name,
        args_node: &SyntaxNode,
        call_site: Span,
        depth: u32,
    ) -> (Option<GreenNode>, Vec<GlyimDiagnostic>) {
        let def = match self.macros.get(&name) {
            Some(d) => d.clone(),
            None => return (None, Vec::new()),
        };

        let args = flatten_token_tree(args_node);

        for arm in &def.arms {
            let result = match_pattern(&arm.pattern, &args);
            match result {
                MatchResult::FullMatch(bindings) => {
                    let expanded = substitution::substitute(&arm.expansion, &bindings);
                    let expanded_green = self.build_expansion_green(&expanded, call_site, depth);
                    return (Some(expanded_green), Vec::new());
                }
                MatchResult::PartialMatch => continue,
                MatchResult::NoMatch => continue,
            }
        }

        let name_str = self.interner.resolve(name);
        (
            None,
            vec![GlyimDiagnostic::type_error(
                call_site,
                format!("no matching macro arm for macro '{}'", name_str),
            )],
        )
    }

    /// Expand a builtin macro.
    fn expand_builtin(
        &mut self,
        handler: BuiltinMacro,
        _args_node: &SyntaxNode,
        call_site: Span,
        _depth: u32,
    ) -> (Option<GreenNode>, Vec<GlyimDiagnostic>) {
        let expanded_trees = match handler {
            BuiltinMacro::File => {
                // file!() expands to a string literal with the file ID
                let file_id_num = call_site.file.to_raw();
                let text = if file_id_num == u32::MAX {
                    // BOGUS file id
                    SmolStr::from("\"<bogus>\"")
                } else {
                    SmolStr::from(format!("\"{}\"", file_id_num))
                };
                vec![TokenTree::Token(SyntaxKind::StringLit, text)]
            }
            BuiltinMacro::Line => {
                // line!() expands to a line number (approximated from byte offset)
                let line_num = call_site
                    .lo
                    .to_raw()
                    .checked_div(80)
                    .unwrap_or(0)
                    .saturating_add(1);
                vec![TokenTree::Token(
                    SyntaxKind::IntLit,
                    SmolStr::from(line_num.to_string()),
                )]
            }
            BuiltinMacro::Column => {
                // column!() expands to a column number (approximated from byte offset)
                let col_num = call_site
                    .lo
                    .to_raw()
                    .checked_rem(80)
                    .unwrap_or(0)
                    .saturating_add(1);
                vec![TokenTree::Token(
                    SyntaxKind::IntLit,
                    SmolStr::from(col_num.to_string()),
                )]
            }
            BuiltinMacro::Env => {
                // env!("VAR") - for now, produce a placeholder or error
                // Try to extract the variable name from args
                return (
                    None,
                    vec![GlyimDiagnostic::type_error(
                        call_site,
                        "env!() macro is not yet fully implemented".to_string(),
                    )],
                );
            }
            BuiltinMacro::Include => {
                // include!("path") - for now, produce an error
                return (
                    None,
                    vec![GlyimDiagnostic::type_error(
                        call_site,
                        "include!() macro is not yet fully implemented".to_string(),
                    )],
                );
            }
        };

        let expanded_green = self.build_expansion_green(&expanded_trees, call_site, _depth);
        (Some(expanded_green), Vec::new())
    }

    fn build_expansion_green(
        &mut self,
        trees: &[TokenTree],
        call_site: Span,
        _depth: u32,
    ) -> GreenNode {
        let expn_id = self.hygiene.push_expansion(ExpnData {
            expn_id: glyim_span::ExpnId::ROOT,
            parent: glyim_span::ExpnId::ROOT,
            kind: ExpnKind::MacroRules {
                name: self.interner.intern("macro_rules"),
            },
            call_site,
            def_site: call_site,
            transparency: Transparency::SemiTransparent,
        });

        let mark = Mark {
            expn_id,
            transparency: Transparency::SemiTransparent,
        };

        let mut builder = rowan::GreenNodeBuilder::new();
        // Wrap expansion tokens in a synthetic SourceFile node so the tree is balanced
        builder.start_node(GlyimLang::kind_to_raw(SyntaxKind::SourceFile));
        for tree in trees {
            self.build_token_tree_green(tree, &mut builder, &mark);
        }
        builder.finish_node();
        builder.finish()
    }

    fn build_token_tree_green(
        &self,
        tree: &TokenTree,
        builder: &mut rowan::GreenNodeBuilder,
        _mark: &Mark,
    ) {
        match tree {
            TokenTree::Token(kind, text) => {
                builder.token(GlyimLang::kind_to_raw(*kind), text.as_str());
            }
            TokenTree::Group(delim_open, children, delim_close) => {
                builder.token(
                    GlyimLang::kind_to_raw(*delim_open),
                    delim_token_text(*delim_open),
                );
                for child in children {
                    self.build_token_tree_green(child, builder, _mark);
                }
                builder.token(
                    GlyimLang::kind_to_raw(*delim_close),
                    delim_token_text(*delim_close),
                );
            }
            TokenTree::DollarCrate => {
                builder.token(GlyimLang::kind_to_raw(SyntaxKind::KwCrate), "crate");
            }
        }
    }

    fn file_id_from_node(&self, _node: &SyntaxNode) -> FileId {
        FileId::BOGUS
    }

    fn span_from_node(&self, node: &SyntaxNode) -> Span {
        let range = node.text_range();
        Span::new(
            self.file_id_from_node(node),
            ByteIdx::from_raw(range.start().into()),
            ByteIdx::from_raw(range.end().into()),
            SyntaxContext::ROOT,
        )
    }
}

fn delim_token_text(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::LParen => "(",
        SyntaxKind::RParen => ")",
        SyntaxKind::LBrace => "{",
        SyntaxKind::RBrace => "}",
        SyntaxKind::LBracket => "[",
        SyntaxKind::RBracket => "]",
        _ => "",
    }
}
