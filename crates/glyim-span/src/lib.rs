//! Source locations, hygiene context, multi-span for diagnostics.

pub mod hygiene;
pub use hygiene::*;

use miette::SourceSpan;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(u32);

impl FileId {
    pub const BOGUS: FileId = FileId(u32::MAX);
    pub fn from_raw(raw: u32) -> Self { Self(raw) }
    pub fn to_raw(self) -> u32 { self.0 }
    pub fn index(self) -> usize { self.0 as usize }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ByteIdx(u32);

impl ByteIdx {
    pub const ZERO: ByteIdx = ByteIdx(0);
    pub fn from_raw(raw: u32) -> Self { Self(raw) }
    pub fn to_raw(self) -> u32 { self.0 }
    pub fn to_usize(self) -> usize { self.0 as usize }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub file: FileId,
    pub lo: ByteIdx,
    pub hi: ByteIdx,
    pub ctx: SyntaxContext,
}

impl Span {
    pub const DUMMY: Span = Span {
        file: FileId(u32::MAX), lo: ByteIdx(0), hi: ByteIdx(0),
        ctx: SyntaxContext::ROOT,
    };

    pub fn new(file: FileId, lo: ByteIdx, hi: ByteIdx, ctx: SyntaxContext) -> Self {
        debug_assert!(lo <= hi, "Span lo > hi");
        Self { file, lo, hi, ctx }
    }

    pub fn is_dummy(self) -> bool { self == Self::DUMMY }
    pub fn range(self) -> Range<usize> { self.lo.to_usize()..self.hi.to_usize() }
    pub fn sans_ctx(self) -> Span { Span { ctx: SyntaxContext::ROOT, ..self } }
    pub fn len(self) -> u32 { self.hi.to_raw().saturating_sub(self.lo.to_raw()) }

    pub fn is_empty(self) -> bool { self.len() == 0 }

    pub fn to(self, other: Span) -> Span {
        debug_assert_eq!(self.file, other.file, "Cannot merge spans from different files");
        Span {
            file: self.file,
            lo: std::cmp::min(self.lo, other.lo),
            hi: std::cmp::max(self.hi, other.hi),
            ctx: self.ctx,
        }
    }
}

impl From<Span> for SourceSpan {
    fn from(s: Span) -> SourceSpan {
        let start = s.lo.to_raw() as usize;
        let length = s.len() as usize;
        SourceSpan::new(start.into(), length)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxContext(u32);

impl SyntaxContext {
    pub const ROOT: SyntaxContext = SyntaxContext(0);
    pub fn is_root(self) -> bool { self.0 == 0 }
    pub fn to_raw(self) -> u32 { self.0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExpnId(u32);

impl ExpnId {
    pub const ROOT: ExpnId = ExpnId(0);
    pub fn is_root(self) -> bool { self.0 == 0 }
    pub fn to_raw(self) -> u32 { self.0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Transparency { Transparent, SemiTransparent, Opaque }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HygieneKey(());

impl HygieneKey {
    pub(crate) fn new() -> Self { HygieneKey(()) }
}

#[derive(Clone, Debug)]
pub struct MultiSpan {
    pub primary: Span,
    pub secondary: Vec<(Span, String)>,
}

impl MultiSpan {
    pub fn from_span(span: Span) -> Self { Self { primary: span, secondary: Vec::new() } }
    pub fn with_secondary(mut self, span: Span, label: impl Into<String>) -> Self {
        self.secondary.push((span, label.into()));
        self
    }
}

impl From<Span> for MultiSpan {
    fn from(span: Span) -> Self { Self::from_span(span) }
}
