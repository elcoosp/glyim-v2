use glyim_diag::GlyimDiagnostic;
use glyim_span::FileId;
use glyim_syntax::{GlyimLang, GreenNode, SyntaxKind, SyntaxNode};
use rowan::Language;

// Submodules containing specific parsing logic
mod expr;
mod item;
mod pat;
mod stmt;
mod ty;

#[derive(Clone, Debug)]
pub struct ParseResult {
    pub green_node: GreenNode,
    pub diagnostics: Vec<GlyimDiagnostic>,
    pub root: SyntaxNode,
}

pub(crate) struct Parser<'a> {
    tokens: &'a [crate::lexer::Token],
    pos: usize,
    builder: rowan::GreenNodeBuilder<'a>,
    diagnostics: Vec<GlyimDiagnostic>,
    last_was_path: bool,
    suppress_struct_lit: bool,
    pending_gt_count: u32,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(tokens: &'a [crate::lexer::Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            builder: rowan::GreenNodeBuilder::new(),
            diagnostics: Vec::new(),
            last_was_path: false,
            suppress_struct_lit: false,
            pending_gt_count: 0,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    fn current(&self) -> Option<&crate::lexer::Token> {
        self.tokens.get(self.pos)
    }

    fn current_kind(&self) -> SyntaxKind {
        if self.pending_gt_count > 0 {
            return SyntaxKind::Gt;
        }
        self.current().map_or(SyntaxKind::Error, |t| t.kind)
    }

    fn bump(&mut self) {
        if self.pending_gt_count > 0 {
            self.builder
                .token(GlyimLang::kind_to_raw(SyntaxKind::Gt), ">");
            self.pending_gt_count -= 1;
            return;
        }
        if let Some(token) = self.current() {
            let kind = GlyimLang::kind_to_raw(token.kind);
            let text = token.text.clone();
            self.builder.token(kind, text.as_str());
            self.pos += 1;
        }
    }

    fn expect(&mut self, expected: SyntaxKind) {
        if self.current_kind() == expected {
            self.bump();
        } else {
            self.error(format!(
                "expected {:?}, found {:?}",
                expected,
                self.current_kind()
            ));
        }
    }

    fn error(&mut self, message: impl Into<String>) {
        let span = self
            .current()
            .map(|t| t.span)
            .unwrap_or(glyim_span::Span::DUMMY);
        self.diagnostics
            .push(GlyimDiagnostic::parse_error(span, message));
    }

    fn bump_expected(&mut self, expected: SyntaxKind) {
        if self.current_kind() != expected {
            self.error(format!(
                "expected {:?}, found {:?}",
                expected,
                self.current_kind()
            ));
        }
        if self.current().is_some() || self.pending_gt_count > 0 {
            self.bump();
        }
    }

    fn checkpoint(&self) -> rowan::Checkpoint {
        self.builder.checkpoint()
    }

    fn start_node_at(&mut self, checkpoint: rowan::Checkpoint, kind: SyntaxKind) {
        self.builder
            .start_node_at(checkpoint, GlyimLang::kind_to_raw(kind));
    }

    fn peek_kind(&self) -> Option<SyntaxKind> {
        self.tokens.get(self.pos + 1).map(|t| t.kind)
    }

    fn skip_token(&mut self) {
        if self.current().is_some() {
            self.pos += 1;
        }
    }

    fn start_node(&mut self, kind: SyntaxKind) {
        let raw_kind = GlyimLang::kind_to_raw(kind);
        self.builder.start_node(raw_kind);
    }

    fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    pub(crate) fn parse_source_file(&mut self) {
        self.start_node(SyntaxKind::SourceFile);
        while self.current().is_some() {
            self.parse_item();
        }
        self.finish_node();
    }

    // Dispatchers to submodules
    // Implementations are in the submodules.

    // ---- FINISH ----

    fn finish(self) -> (GreenNode, Vec<GlyimDiagnostic>) {
        (self.builder.finish(), self.diagnostics)
    }
}

pub fn parse_to_syntax(source: &str, file_id: FileId) -> ParseResult {
    let lex_result = crate::lexer::lex(source, file_id);
    let mut parser = Parser::new(&lex_result.tokens);
    parser.parse_source_file();
    let (green_node, diagnostics) = parser.finish();
    let root = SyntaxNode::new_root(green_node.clone());
    let mut all_diagnostics = lex_result.diagnostics;
    all_diagnostics.extend(diagnostics);
    ParseResult {
        green_node,
        diagnostics: all_diagnostics,
        root,
    }
}

/// Validate whether `src` forms a single, whole, well-formed fragment of the
/// requested kind. Used by the macro matcher's Stage B to consume
/// variable-length fragments (e.g. a full `a + b * c` for `:expr`) instead of
/// a single token tree.
///
/// Returns `Some(())` when `src` parses as exactly one valid fragment of the
/// given kind (and only that), `None` otherwise. Parse *errors* are tolerated
/// as long as the structural shape is correct — the macro expander will surface
/// real diagnostics later. This keeps fragment matching permissive about
/// unfinished expressions while still rejecting structurally invalid input.
pub fn try_parse_fragment(kind: &str, src: &str) -> Option<()> {
    use glyim_syntax::SyntaxKind;
    let file_id = FileId::from_raw(0);
    // Wrap the fragment in a context that makes it a single top-level construct
    // of the requested kind, then inspect the parse tree.
    let wrapped = match kind {
        "expr" | "stmt" => format!("fn __f() {{ {} }}", src),
        // For `:block` the fragment *is* the function body — if it parses as a
        // bare block, it becomes `__f`'s body directly.
        "block" => format!("fn __f() {}", src),
        "ty" => format!("fn __f(__x: {}) {{ }}", src),
        "pat" => format!("fn __f() {{ let {} = 0; }}", src),
        "item" => format!("fn __f() {{ {} }}", src),
        "meta" => format!("#[{}] fn __f() {{ }}", src),
        // Unknown spec — fall back to permissive single-token acceptance.
        _ => return None,
    };
    let parsed = parse_to_syntax(&wrapped, file_id);
    // A fragment is only valid if it parses *cleanly* — no diagnostics. This
    // rejects over-consumed tokens (e.g. a stray `)` from greedy matching) and
    // malformed fragments, which is exactly what Stage B needs.
    if !parsed.diagnostics.is_empty() {
        return None;
    }
    let root = parsed.root;
    let fndef = root
        .children()
        .find(|n| n.kind() == SyntaxKind::FnDef)?;
    match kind {
        "expr" => {
            // Exactly one expression-bearing child (ExprStmt / LetStmt / tail expr).
            let block = fndef
                .children()
                .find(|n| n.kind() == SyntaxKind::Block)?;
            let stmts: Vec<_> = block
                .children()
                .filter(|n| {
                    matches!(
                        n.kind(),
                        SyntaxKind::ExprStmt | SyntaxKind::LetStmt
                    )
                })
                .collect();
            // Allow a bare tail expression (block with a single non-statement
            // expr child) or a single statement.
            let has_expr = stmts.len() == 1
                || (stmts.is_empty()
                    && block.children().any(|n| is_exprish(n.kind())));
            has_expr.then_some(())
        }
        "stmt" => {
            let block = fndef
                .children()
                .find(|n| n.kind() == SyntaxKind::Block)?;
            let stmts: Vec<_> = block
                .children()
                .filter(|n| {
                    matches!(
                        n.kind(),
                        SyntaxKind::ExprStmt | SyntaxKind::LetStmt
                    )
                })
                .collect();
            (stmts.len() == 1).then_some(())
        }
        "block" => {
            // `<src>` is the function body itself. A `:block` fragment is valid
            // when it parses as exactly one block (brace-delimited) and nothing
            // else. We don't compare source text exactly (whitespace inside the
            // block is insignificant), only that a body block exists and the
            // fragment is brace-delimited.
            let _block = fndef
                .children()
                .find(|n| n.kind() == SyntaxKind::Block)?;
            let src_t = src.trim();
            (src_t.starts_with('{') && src_t.ends_with('}') && src_t.len() >= 2)
                .then_some(())
        }
        "ty" => {
            let param = fndef
                .children()
                .find(|n| n.kind() == SyntaxKind::ParamList)?
                .children()
                .find(|n| n.kind() == SyntaxKind::Param)?;
            // Param has a type child that is a recognised type node.
            let has_ty = param.children().any(|n| is_typeish(n.kind()));
            has_ty.then_some(())
        }
        "pat" => {
            // `<src>` is bound in `let <src> = 0;`. Find the LetStmt and check
            // its pattern child is a recognised pattern shape.
            let let_stmt = fndef
                .children()
                .find(|n| n.kind() == SyntaxKind::Block)?
                .children()
                .find(|n| n.kind() == SyntaxKind::LetStmt)?;
            let has_pat = let_stmt.children().any(|n| is_patish(n.kind()));
            has_pat.then_some(())
        }
        "item" => {
            let block = fndef
                .children()
                .find(|n| n.kind() == SyntaxKind::Block)?;
            let items: Vec<_> = block
                .children()
                .filter(|n| {
                    matches!(
                        n.kind(),
                        SyntaxKind::FnDef
                            | SyntaxKind::StructDef
                            | SyntaxKind::EnumDef
                            | SyntaxKind::ImplDef
                            | SyntaxKind::ExternBlock
                    )
                })
                .collect();
            (items.len() == 1).then_some(())
        }
        "meta" => {
            // Attributes aren't represented as a distinct syntax node in this
            // frontend, so we accept any non-empty `meta` content. This keeps
            // `:meta` permissive (as Stage A did) without a fake structural
            // check. The expander validates semantics later.
            let content = src.trim();
            if content.is_empty() {
                None
            } else {
                Some(())
            }
        }
        _ => None,
    }
}

/// Expression-like node kinds that can stand as a bare `:expr` tail.
fn is_exprish(kind: SyntaxKind) -> bool {
    use glyim_syntax::SyntaxKind;
    matches!(
        kind,
        SyntaxKind::PathExpr
            | SyntaxKind::LitExpr
            | SyntaxKind::BinaryExpr
            | SyntaxKind::CallExpr
            | SyntaxKind::MethodCallExpr
            | SyntaxKind::IfExpr
            | SyntaxKind::MatchExpr
            | SyntaxKind::Block
            | SyntaxKind::UnaryExpr
            | SyntaxKind::RefExpr
            | SyntaxKind::TupleExpr
            | SyntaxKind::ArrayExpr
            | SyntaxKind::StructExpr
            | SyntaxKind::CastExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::FieldExpr
            | SyntaxKind::RangeExpr
            | SyntaxKind::ClosureExpr
            | SyntaxKind::WhileExpr
            | SyntaxKind::LoopExpr
            | SyntaxKind::ForExpr
            | SyntaxKind::ReturnExpr
            | SyntaxKind::BreakExpr
            | SyntaxKind::ContinueExpr
            | SyntaxKind::AssignExpr
    )
}

/// Type-like node kinds for `:ty` validation.
fn is_typeish(kind: SyntaxKind) -> bool {
    use glyim_syntax::SyntaxKind;
    matches!(
        kind,
        SyntaxKind::PathType
            | SyntaxKind::TupleType
            | SyntaxKind::ArrayType
            | SyntaxKind::RefType
            | SyntaxKind::RawPtrType
            | SyntaxKind::FnType
            | SyntaxKind::DynType
            | SyntaxKind::NeverType
            | SyntaxKind::SliceType
            | SyntaxKind::ImplTraitType
            | SyntaxKind::InferType
    )
}

/// Pattern-like node kinds for `:pat` validation.
fn is_patish(kind: SyntaxKind) -> bool {
    use glyim_syntax::SyntaxKind;
    matches!(
        kind,
        SyntaxKind::PatIdent
            | SyntaxKind::PatWild
            | SyntaxKind::PatLit
            | SyntaxKind::PatRange
            | SyntaxKind::PatTuple
            | SyntaxKind::PatStruct
            | SyntaxKind::PatOr
            | SyntaxKind::PatSlice
    )
}
