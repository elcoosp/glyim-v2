use glyim_span::FileId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use parking_lot::RwLock;
use crate::symbol_index::SymbolIndex;
use crate::reference_graph::ReferenceGraph;

#[derive(Clone)]
pub struct SourceMap {
    path: PathBuf,
    file_id: FileId,
    content: String,
}

impl SourceMap {
    pub fn new(path: PathBuf, file_id: FileId, content: String) -> Self {
        Self { path, file_id, content }
    }
    pub fn file_id(&self) -> FileId { self.file_id }
    pub fn source(&self) -> &str { &self.content }
    pub fn span_to_position(&self, _lo: usize, _hi: usize) -> Option<((usize, usize), (usize, usize))> {
        Some(((0, 0), (0, 0)))
    }
    pub fn line_col_to_offset(&self, _line: usize, _col: usize) -> Option<usize> {
        Some(0)
    }
}

pub struct FileMap {
    path_to_id: HashMap<PathBuf, FileId>,
    id_to_path: HashMap<FileId, PathBuf>,
    next_id: u32,
}

impl Default for FileMap {
    fn default() -> Self { Self::new() }
}

impl FileMap {
    pub fn new() -> Self {
        Self { path_to_id: HashMap::new(), id_to_path: HashMap::new(), next_id: 0 }
    }
    pub fn get_or_create(&mut self, path: &PathBuf) -> FileId {
        if let Some(id) = self.path_to_id.get(path) {
            return *id;
        }
        let id = FileId::from_raw(self.next_id);
        self.next_id += 1;
        self.path_to_id.insert(path.clone(), id);
        self.id_to_path.insert(id, path.clone());
        id
    }
    pub fn get_by_path(&self, path: &Path) -> Option<FileId> {
        self.path_to_id.get(path).copied()
    }
    pub fn path(&self, id: FileId) -> Option<&PathBuf> {
        self.id_to_path.get(&id)
    }
    pub fn remove(&mut self, path: &PathBuf) {
        if let Some(id) = self.path_to_id.remove(path) {
            self.id_to_path.remove(&id);
        }
    }
}

pub struct AnalysisDatabase {
    pub file_map: RwLock<FileMap>,
    pub source_maps: RwLock<HashMap<FileId, SourceMap>>,
    pub symbol_index: RwLock<SymbolIndex>,
    pub reference_graph: RwLock<ReferenceGraph>,
    pub hirs: RwLock<HashMap<FileId, glyim_hir::CrateHir>>,
    pub diagnostics: RwLock<HashMap<FileId, lsp_types::Diagnostic>>,
    pub file_access_times: RwLock<HashMap<FileId, Instant>>,
}

impl Default for AnalysisDatabase {
    fn default() -> Self { Self::new() }
}

impl AnalysisDatabase {
    pub fn new() -> Self {
        Self {
            file_map: RwLock::new(FileMap::new()),
            source_maps: RwLock::new(HashMap::new()),
            symbol_index: RwLock::new(SymbolIndex::new()),
            reference_graph: RwLock::new(ReferenceGraph::new()),
            hirs: RwLock::new(HashMap::new()),
            diagnostics: RwLock::new(HashMap::new()),
            file_access_times: RwLock::new(HashMap::new()),
        }
    }

    pub fn touch(&self, _file_id: FileId) {}
    pub fn evict_stale(&self, _max_age: std::time::Duration) {}
}
