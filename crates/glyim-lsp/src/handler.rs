use crate::code_action::provide_code_actions;
use crate::completion::provide_completions;
use crate::database::FileMap;
use crate::driver::AnalysisMessage;
use crate::folding::provide_folding_ranges;
use crate::formatting::format_document;
use crate::goto_definition::goto_definition;
use crate::hover::provide_hover;
use crate::navigation::{document_symbols, find_references};
use crate::rename::rename_symbol;
use crate::AnalysisDatabase;
use async_lsp::router::Router;
use lsp_types::request::{
    CodeActionRequest, Completion, DocumentSymbolRequest, FoldingRangeRequest, Formatting,
    GotoDefinition, HoverRequest, Initialize, References, Rename, Shutdown,
};
use lsp_types::*;
use std::sync::Arc;
use tokio::sync::mpsc;

pub fn build_router(
    db: Arc<AnalysisDatabase>,
    analysis_tx: mpsc::Sender<AnalysisMessage>,
    _client: async_lsp::ClientSocket,
) -> Router<()> {
    let mut router = Router::new(());
    let file_map = Arc::new(parking_lot::RwLock::new(FileMap::new()));

    // Initialize
    let db_init = db.clone();
    router.request::<Initialize, _>(move |_, params: InitializeParams| {
        let db = db_init.clone();
        async move {
            let capabilities = ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        ..Default::default()
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            };
            Ok(InitializeResult {
                capabilities,
                server_info: None,
            })
        }
    });

    // Shutdown
    router.request::<Shutdown, _>(move |_, _: ()| async move { Ok(()) });

    // Completion
    let db_comp = db.clone();
    let file_map_comp = file_map.clone();
    router.request::<Completion, _>(move |_, params: CompletionParams| {
        let db = db_comp.clone();
        let file_map = file_map_comp.clone();
        async move {
            let guard = file_map.read();
            Ok(provide_completions(&db, &guard, &params))
        }
    });

    // Hover
    let db_hover = db.clone();
    let file_map_hover = file_map.clone();
    router.request::<HoverRequest, _>(move |_, params: HoverParams| {
        let db = db_hover.clone();
        let file_map = file_map_hover.clone();
        async move {
            let guard = file_map.read();
            Ok(provide_hover(&db, &guard, &params))
        }
    });

    // Goto Definition
    let db_def = db.clone();
    let file_map_def = file_map.clone();
    router.request::<GotoDefinition, _>(move |_, params: GotoDefinitionParams| {
        let db = db_def.clone();
        let file_map = file_map_def.clone();
        async move {
            let guard = file_map.read();
            Ok(goto_definition(&db, &guard, &params))
        }
    });

    // Find References
    let db_ref = db.clone();
    let file_map_ref = file_map.clone();
    router.request::<References, _>(move |_, params: ReferenceParams| {
        let db = db_ref.clone();
        let file_map = file_map_ref.clone();
        async move {
            let guard = file_map.read();
            Ok(find_references(&db, &guard, &params))
        }
    });

    // Formatting
    let db_fmt = db.clone();
    let file_map_fmt = file_map.clone();
    router.request::<Formatting, _>(move |_, params: DocumentFormattingParams| {
        let db = db_fmt.clone();
        let file_map = file_map_fmt.clone();
        async move {
            let guard = file_map.read();
            Ok(format_document(&db, &params))
        }
    });

    // Rename
    let db_rename = db.clone();
    let file_map_rename = file_map.clone();
    router.request::<Rename, _>(move |_, params: RenameParams| {
        let db = db_rename.clone();
        let file_map = file_map_rename.clone();
        async move {
            let guard = file_map.read();
            Ok(rename_symbol(&db, &guard, &params))
        }
    });

    // FoldingRange - using FoldingRangeRequest
    let db_fold = db.clone();
    router.request::<FoldingRangeRequest, _>(move |_, params: FoldingRangeParams| {
        let db = db_fold.clone();
        async move { Ok(provide_folding_ranges(&db, &params)) }
    });

    // Code Action
    let db_action = db.clone();
    let file_map_action = file_map.clone();
    router.request::<CodeActionRequest, _>(move |_, params: CodeActionParams| {
        let db = db_action.clone();
        let file_map = file_map_action.clone();
        async move {
            let guard = file_map.read();
            Ok(provide_code_actions(&db, &guard, &params))
        }
    });

    // Document Symbols
    let db_doc = db.clone();
    let file_map_doc = file_map;
    router.request::<DocumentSymbolRequest, _>(move |_, params: DocumentSymbolParams| {
        let db = db_doc.clone();
        let file_map = file_map_doc.clone();
        async move {
            let guard = file_map.read();
            Ok(document_symbols(&db, &guard, &params))
        }
    });

    router
}
