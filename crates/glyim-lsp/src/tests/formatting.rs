use crate::database::{FileMap, SourceMap};
use crate::formatting::format_document;
use crate::AnalysisDatabase;
use lsp_types::*;
use std::fs;
use std::path::PathBuf;
use tempfile::NamedTempFile;

fn setup_analysis_with_temp_file(content: &str) -> (AnalysisDatabase, FileMap, PathBuf, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_path_buf();
    fs::write(&path, content).unwrap();
    let db = AnalysisDatabase::new();
    let mut file_map = FileMap::new();
    let file_id = file_map.get_or_create(&path);
    let source_map = SourceMap::new(path.clone(), file_id, content.to_string());
    db.source_maps.write().insert(file_id, source_map);
    (db, file_map, path, temp_file)
}

#[test]
fn test_format_document() {
    let content = "fn main(){let x=1;}";
    let (db, _file_map, path, _temp_file) = setup_analysis_with_temp_file(content);
    let uri = Url::from_file_path(&path).unwrap();
    let params = DocumentFormattingParams {
        text_document: TextDocumentIdentifier { uri },
        options: FormattingOptions::default(),
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    let edits = format_document(&db, &params);
    assert!(edits.is_some());
    let edits = edits.unwrap();
    assert!(!edits.is_empty());
    let first_edit = &edits[0];
    assert_eq!(first_edit.range.start.line, 0);
    assert_eq!(first_edit.range.start.character, 0);
    assert!(first_edit.new_text.contains(' '));
}
