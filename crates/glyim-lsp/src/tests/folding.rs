use crate::AnalysisDatabase;
use std::str::FromStr;
use lsp_types::Uri;
use url::Url;
use crate::database::SourceMap;
use crate::folding::provide_folding_ranges;
use lsp_types::*;

fn setup_analysis(content: &str) -> (AnalysisDatabase, Uri) {
    let db = AnalysisDatabase::new();
    let path = std::env::current_dir().unwrap().join("main.gly");
    let uri = Uri::from_str(&Uri::from_str(&Url::from_file_path(&path).unwrap().to_string()).unwrap().to_string()).unwrap();
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