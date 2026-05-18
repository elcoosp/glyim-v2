use crate::{SymbolIndex, SymbolKind};
use glyim_core::{IndexVec, Interner, Name, Visibility};
use glyim_hir::*;
use glyim_span::{FileId, Span, ByteIdx, SyntaxContext};

fn create_dummy_hir(interner: &Interner) -> CrateHir {
    let mut items = IndexVec::new();
    let key = interner.intern("main");
    let name = Name::from(key);
    let fn_item = Item {
        id: ItemId::from_raw(0),
        name,
        kind: ItemKind::Fn(FnItem {
            params: vec![],
            return_ty: None,
            body: None,
            is_unsafe: false,
            is_async: false,
            generic_params: vec![],
            where_clauses: vec![],
        }),
        visibility: Visibility::Public,
        span: Span::new(FileId::from_raw(1), ByteIdx::ZERO, ByteIdx::from_raw(10), SyntaxContext::ROOT),
    };
    items.push(fn_item);
    CrateHir {
        items,
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
    }
}

#[test]
fn build_from_hir_adds_function_symbol() {
    let mut index = SymbolIndex::new();
    let file_id = FileId::from_raw(1);
    let interner = Interner::default();
    let hir = create_dummy_hir(&interner);
    index.build_from_hir(file_id, &hir, &interner);
    let symbols = index.symbols_in_file(file_id);
    assert_eq!(symbols.len(), 1);
    // The name stored in SymbolInfo is a String directly from interner.resolve
    assert_eq!(symbols[0].name, "main");
    assert_eq!(symbols[0].kind, SymbolKind::Function);
}
