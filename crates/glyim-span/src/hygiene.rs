use crate::{ExpnId, HygieneKey, Span, SyntaxContext, Transparency};
use glyim_core::interner::Name;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Mark {
    pub expn_id: ExpnId,
    pub transparency: Transparency,
}

#[derive(Clone, Debug)]
pub struct ExpnData {
    pub expn_id: ExpnId,
    pub parent: ExpnId,
    pub kind: ExpnKind,
    pub call_site: Span,
    pub def_site: Span,
    pub transparency: Transparency,
}

#[derive(Clone, Debug)]
pub enum ExpnKind {
    MacroRules { name: Name },
    ProcMacro { name: Name },
    Builtin { name: Name },
    Root,
}

#[derive(Clone, Debug)]
struct SyntaxContextData {
    outer_expn: ExpnId,
    outer_transparency: Transparency,
    parent: SyntaxContext,
}

pub struct HygieneCtx {
    expansions: Vec<ExpnData>,
    next_expn_id: u32,
    syntax_contexts: Vec<SyntaxContextData>,
    next_syntax_context: u32,
    key: HygieneKey,
    adjust_warned: bool,
}

impl HygieneCtx {
    pub fn new() -> Self {
        Self {
            expansions: vec![ExpnData {
                expn_id: ExpnId::ROOT,
                parent: ExpnId::ROOT,
                kind: ExpnKind::Root,
                call_site: Span::DUMMY,
                def_site: Span::DUMMY,
                transparency: Transparency::Opaque,
            }],
            next_expn_id: 1,
            syntax_contexts: Vec::new(),
            next_syntax_context: 1,
            key: HygieneKey::new(),
            adjust_warned: false,
        }
    }

    pub fn push_expansion(&mut self, mut data: ExpnData) -> ExpnId {
        let raw_id = self.next_expn_id;
        self.next_expn_id += 1;
        let id = ExpnId::from_hygiene_key(self.key, raw_id);
        data.expn_id = id;
        self.expansions.push(data);
        id
    }

    pub fn apply_mark(&mut self, span: Span, mark: Mark) -> Span {
        let new_ctx = SyntaxContext::from_hygiene_key(self.key, self.next_syntax_context);
        self.next_syntax_context += 1;
        self.syntax_contexts.push(SyntaxContextData {
            outer_expn: mark.expn_id,
            outer_transparency: mark.transparency,
            parent: span.ctx,
        });
        Span::new(span.file, span.lo, span.hi, new_ctx)
    }

    pub fn remove_mark(&self, span: Span) -> (Span, Option<Mark>) {
        if span.ctx.is_root() {
            return (span, None);
        }
        let idx = span.ctx.to_raw() as usize - 1;
        if let Some(ctx_data) = self.syntax_contexts.get(idx) {
            let mark = Mark {
                expn_id: ctx_data.outer_expn,
                transparency: ctx_data.outer_transparency,
            };
            (
                Span::new(span.file, span.lo, span.hi, ctx_data.parent),
                Some(mark),
            )
        } else {
            (span, None)
        }
    }

    pub fn expn_data(&self, id: ExpnId) -> Option<&ExpnData> {
        self.expansions.get(id.to_raw() as usize)
    }

    pub fn adjust(&mut self, span: Span, _scope_ctx: SyntaxContext) -> Span {
        if !self.adjust_warned {
            self.adjust_warned = true;
            tracing::warn!("hygiene adjust is stubbed; macro spans may be incorrect.");
        }
        span
    }
}

impl Default for HygieneCtx {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxContext {
    fn from_hygiene_key(_key: HygieneKey, raw: u32) -> Self {
        let _ = _key;
        SyntaxContext(raw)
    }
}

impl ExpnId {
    fn from_hygiene_key(_key: HygieneKey, raw: u32) -> Self {
        let _ = _key;
        ExpnId(raw)
    }
}
