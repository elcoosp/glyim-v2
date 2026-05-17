use crate::database::SourceMap;
use crate::folding::provide_folding_ranges;
use crate::AnalysisDatabase;
use lsp_types::*;

fn setup_analysis(content: &str) -> (AnalysisDatabase, Url) {
    let db = AnalysisDatabase::new();
    let path = std::env::current_dir().unwrap().join("main.gly");
    let uri = Url::from_file_path(&path).unwrap();
    let file_id = db.file_map.write().get_or_create(&path);
    let source_map = SourceMap::new(path, file_id, content.to_string());
    db.source_maps.write().insert(file_id, source_map);
    (db, uri)
}

#[test]
fn test_folding_ranges_for_braces() {
    let content = r#"fn main() {
    let x = 1;
    if x > 0 {
        println!("hello");
    }
}"#;
    let (db, uri) = setup_analysis(content);
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
