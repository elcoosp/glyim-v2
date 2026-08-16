//! Tier 6.4: LSP-side receiver-type completion filtering.
//!
//! Verifies (a) that `impl` methods are indexed with their receiver type, and
//! (b) that `provide_completions` narrows to methods whose receiver type
//! matches the resolved type of the expression before a `.` trigger.

use crate::completion::provide_completions;
use crate::database::{AnalysisDatabase, SourceMap};
use crate::symbol_index::{DefinitionLocation, SymbolInfo, SymbolKind, TypeSignature};
use glyim_core::primitives::IntTy;
use glyim_core::{Interner, LocalDefId};
use glyim_hir::{Body, CrateHir, ExprId};
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_type::{TyCtxMut, TyKind};
use glyim_typeck::TypeckResult;
use lsp_types::{
    CompletionContext, CompletionParams, CompletionResponse, CompletionTriggerKind, Position,
    TextDocumentIdentifier, TextDocumentPositionParams, Uri,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use url::Url;

fn uri_for(path: &PathBuf) -> Uri {
    Uri::from_str(Url::from_file_path(path).unwrap().as_ref()).unwrap()
}

fn completion_params(path: &PathBuf, line: u32, character: u32) -> CompletionParams {
    CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri_for(path) },
            position: Position { line, character },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: Some(CompletionContext {
            trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
            trigger_character: Some(".".to_string()),
        }),
    }
}

#[test]
fn s12_impl_methods_indexed_with_receiver_type() {
    let source = "struct Foo {}\nimpl Foo {\n    fn ping(&self) {}\n    fn pong(&self) {}\n}\n";
    let db = AnalysisDatabase::new();
    let path = PathBuf::from("/test/receiver.g");
    let file_id = db.file_map.write().get_or_create(&path);
    let mut interner = Interner::new();
    let parse_result = glyim_frontend::parse_to_syntax(source, file_id);
    let (hir, _hir_diags) =
        glyim_hir::pipeline_api::lower_crate_for_pipeline(&parse_result.root, &mut interner);
    db.symbol_index
        .write()
        .build_from_hir(file_id, &hir, &interner);

    let guard = db.symbol_index.read();
    let methods: Vec<&SymbolInfo> = guard
        .symbols_in_file(file_id)
        .into_iter()
        .filter(|s| s.name == "ping" || s.name == "pong")
        .collect();
    assert_eq!(methods.len(), 2, "expected both impl methods to be indexed");
    for m in &methods {
        assert_eq!(
            m.type_signature
                .as_ref()
                .and_then(|ts| ts.receiver_type.clone()),
            Some("Foo".to_string()),
            "impl method should carry receiver type `Foo`"
        );
    }
}

#[test]
fn s12_dot_completion_filters_by_receiver_type() {
    // Deterministic end-to-end check of the Tier 6.4 receiver-type filter.
    // We build the database directly (rather than via the parser) because the
    // current frontend does not lower `x.foo()` method-call receivers, so a
    // real `analyze()` cannot yet produce a receiver expr at a `.`. The filter
    // logic itself is what we verify here: a `.` completion at a position whose
    // resolved receiver type is `i32` must surface only `i32` methods and drop
    // free functions.
    let source = "x.\n";
    let db = AnalysisDatabase::new();
    let path = PathBuf::from("/test/receiver.g");
    let file_id = db.file_map.write().get_or_create(&path);
    db.source_maps.write().insert(
        file_id,
        SourceMap::new(path.clone(), file_id, source.to_string()),
    );

    // Record a receiver expression (`x`) whose span ends right before the `.`.
    let dot = source.find('.').expect("`.` present") as u32;
    let x_span = Span::new(
        file_id,
        ByteIdx::from_raw(0),
        ByteIdx::from_raw(dot),
        SyntaxContext::ROOT,
    );
    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs: Default::default(),
        pats: Default::default(),
        params: Vec::new(),
        span: x_span,
        expr_spans: {
            let mut v = glyim_core::arena::IndexVec::new();
            v.push(x_span);
            v
        },
    };
    let hir = CrateHir {
        items: Default::default(),
        bodies: {
            let mut v = glyim_core::arena::IndexVec::new();
            v.push(body);
            v
        },
        body_owners: {
            let mut v = glyim_core::arena::IndexVec::new();
            v.push(LocalDefId::from_raw(0));
            v
        },
    };
    db.hirs.write().insert(file_id, hir);

    // Resolve the receiver expr to `i32` via a type-checking result.
    let mut ty_ctx_mut = TyCtxMut::new(Interner::new());
    let i32_ty = ty_ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let ty_ctx = Arc::new(ty_ctx_mut.freeze());
    let typeck_result = TypeckResult {
        thir_bodies: Vec::new(),
        diagnostics: Vec::new(),
        expr_types: HashMap::from([(
            LocalDefId::from_raw(0),
            HashMap::from([(ExprId::from_raw(0), i32_ty)]),
        )]),
    };
    db.typeck.write().insert(file_id, (ty_ctx, typeck_result));

    // Index an `i32` method and a free function.
    let method = SymbolInfo {
        name: "ping".to_string(),
        kind: SymbolKind::Function,
        definition: DefinitionLocation {
            file_id,
            span: Span::DUMMY,
        },
        type_signature: Some(TypeSignature {
            params: Vec::new(),
            return_type: None,
            receiver_type: Some("i32".to_string()),
        }),
        is_pub: false,
        documentation: None,
    };
    let free_fn = SymbolInfo {
        name: "unrelated".to_string(),
        kind: SymbolKind::Function,
        definition: DefinitionLocation {
            file_id,
            span: Span::DUMMY,
        },
        type_signature: Some(TypeSignature {
            params: Vec::new(),
            return_type: None,
            receiver_type: None,
        }),
        is_pub: false,
        documentation: None,
    };
    {
        let mut idx = db.symbol_index.write();
        idx.insert_test_symbol(file_id, method);
        idx.insert_test_symbol(file_id, free_fn);
    }

    // Cursor placed right AFTER the `.` (the `.` trigger means
    // `src[..offset]` ends with `.`).
    let offset = (dot as usize) + 1;
    let (line, character) = line_col_of(source, offset);

    let params = completion_params(&path, line, character);
    let file_map_guard = db.file_map.read();
    let result = provide_completions(&db, &file_map_guard, &params);
    drop(file_map_guard);
    let items = match result {
        Some(CompletionResponse::List(list)) => list.items,
        other => panic!("expected a completion list, got {:?}", other),
    };
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.contains(&"ping"),
        "expected `ping` (receiver i32) in completions, got {:?}",
        labels
    );
    assert!(
        !labels.contains(&"unrelated"),
        "free function `unrelated` must be filtered out at a `.` call site, got {:?}",
        labels
    );
}

fn line_col_of(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}
