use crate::database::{AnalysisDatabase, FileMap, SourceMap};
use glyim_span::FileId;
use lsp_types::Url;
use std::path::PathBuf;

pub fn setup_test_db(content: &str, path_str: &str) -> (AnalysisDatabase, FileMap, Url, FileId) {
    let db = AnalysisDatabase::new();
    let mut file_map = FileMap::new();
    let path = PathBuf::from(path_str);
    let uri = Url::from_file_path(&path).unwrap();
    let file_id = file_map.get_or_create(&path);
    // Also insert into db's file_map so functions that read db.file_map can find it
    db.file_map.write().get_or_create(&path);
    let source_map = SourceMap::new(path.clone(), file_id, content.to_string());
    db.source_maps.write().insert(file_id, source_map);
    (db, file_map, uri, file_id)
}
