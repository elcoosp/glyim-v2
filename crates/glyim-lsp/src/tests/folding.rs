use crate::database::{FileMap, SourceMap};
use crate::folding::provide_folding_ranges;
use crate::AnalysisDatabase;
use lsp_types::*;
use std::path::PathBuf;

fn setup_analysis(content: &str, path: &PathBuf) -> (AnalysisDatabase, FileMap) {
    let db = AnalysisDatabase::new();
    let mut file_map = FileMap::new();
    let file_id = file_map.get_or_create(path);
    let source_map = SourceMap::new(path.clone(), file_id, content.to_string());
    db.source_maps.write().insert(file_id, source_map);
    (db, file_map)
}

#[test]
fn test_folding_ranges_for_braces() {
    let content = r#"fn main() {
    let x = 1;
    if x > 0 {
        println!("hello");
    }
}"#;
    let path = PathBuf::from("main.gly");
    let (db, _file_map) = setup_analysis(content, &path);
    let uri = Url::from_file_path(path).unwrap();
    let params = FoldingRangeParams {
        text_document: TextDocumentIdentifier { uri },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let ranges = provide_folding_ranges(&db, &params);
    assert!(ranges.is_some());
    let ranges = ranges.unwrap();
    assert!(ranges.len() >= 2);
}
