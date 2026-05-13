pub mod format;

static SNAPSHOT_FILE_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1000);

fn next_snapshot_file_id() -> glyim_span::FileId {
    glyim_span::FileId::from_raw(
        SNAPSHOT_FILE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    )
}

pub fn snapshot_cst(name: &str, source: &str) {
    let file_id = next_snapshot_file_id();
    let result = glyim_frontend::parse_to_syntax(source, file_id);
    let tree = format!("{:#?}", result.root);
    insta::with_settings!({ snapshot_suffix => name }, {
        insta::assert_snapshot!(tree);
    });
}

pub fn snapshot_mir(name: &str, ctx: &glyim_type::TyCtx, body: &glyim_mir::Body) {
    let formatted = format::format_mir_body(ctx, body);
    insta::with_settings!({ snapshot_suffix => name }, {
        insta::assert_snapshot!(formatted);
    });
}

pub fn snapshot_def_map(name: &str, def_map: &glyim_def_map::CrateDefMap) {
    let formatted = format::format_def_map(def_map);
    insta::with_settings!({ snapshot_suffix => name }, {
        insta::assert_snapshot!(formatted);
    });
}
