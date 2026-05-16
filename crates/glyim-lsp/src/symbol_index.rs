use glyim_span::{FileId, Span};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub definition: DefinitionLocation,
    pub type_signature: Option<TypeSignature>,
    pub is_pub: bool,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function, Struct, Enum, EnumVariant, Field, TypeParameter, Local, Module,
}

#[derive(Debug, Clone)]
pub struct DefinitionLocation {
    pub file_id: FileId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeSignature {
    pub params: Vec<(String, String)>,
    pub return_type: Option<String>,
}

pub struct SymbolIndex {
    by_name: HashMap<String, Vec<SymbolInfo>>,
    by_file: HashMap<FileId, Vec<SymbolInfo>>,
    by_location: HashMap<(u32, usize), SymbolInfo>,
}

impl Default for SymbolIndex {
    fn default() -> Self { Self::new() }
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self { by_name: HashMap::new(), by_file: HashMap::new(), by_location: HashMap::new() }
    }

    pub fn build_from_hir(&mut self, _file_id: FileId, _hir: &glyim_hir::CrateHir, _interner: &glyim_core::Interner) {
        tracing::warn!("STUB: SymbolIndex::build_from_hir");
    }

    pub fn lookup_by_name(&self, name: &str) -> Vec<&SymbolInfo> {
        self.by_name.get(name).map(|v| v.iter().collect()).unwrap_or_default()
    }

    pub fn lookup_by_location(&self, file_id: FileId, offset: usize) -> Option<&SymbolInfo> {
        self.by_location.get(&(file_id.to_raw(), offset))
    }

    pub fn symbols_in_file(&self, file_id: FileId) -> Vec<&SymbolInfo> {
        self.by_file.get(&file_id).map(|v| v.iter().collect()).unwrap_or_default()
    }

    pub fn query(&self, prefix: &str, limit: usize) -> Vec<&SymbolInfo> {
        let mut results = Vec::new();
        for (name, symbols) in &self.by_name {
            if name.starts_with(prefix) && results.len() < limit {
                results.extend(symbols.iter().take(limit - results.len()));
            }
        }
        if results.is_empty() {
            for (name, symbols) in &self.by_name {
                if name.contains(prefix) && results.len() < limit {
                    results.extend(symbols.iter().take(limit - results.len()));
                }
            }
        }
        results
    }

    pub fn clear_file(&mut self, file_id: FileId) {
        if let Some(symbols) = self.by_file.remove(&file_id) {
            for sym in symbols {
                if let Some(entries) = self.by_name.get_mut(&sym.name) {
                    entries.retain(|s| s.definition.file_id != file_id);
                    if entries.is_empty() {
                        self.by_name.remove(&sym.name);
                    }
                }
                self.by_location.remove(&(file_id.to_raw(), sym.definition.span.lo.to_usize()));
            }
        }
    }

    #[doc(hidden)]
    pub fn insert_test_symbol(&mut self, file_id: FileId, sym: SymbolInfo) {
        self.by_name.entry(sym.name.clone()).or_default().push(sym.clone());
        self.by_file.entry(file_id).or_default().push(sym.clone());
        self.by_location.insert((file_id.to_raw(), sym.definition.span.lo.to_usize()), sym);
    }
}
