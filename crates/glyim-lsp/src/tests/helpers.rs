use glyim_span::{FileId, Span, ByteIdx, SyntaxContext};

pub fn make_span(file_id: FileId, lo: usize, hi: usize) -> Span {
    Span::new(
        file_id,
        ByteIdx::from_raw(lo as u32),
        ByteIdx::from_raw(hi as u32),
        SyntaxContext::ROOT,
    )
}
