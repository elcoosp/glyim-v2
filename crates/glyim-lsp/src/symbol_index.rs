use glyim_core::{Interner, LocalDefId};
use glyim_hir::{Body, CrateHir, ItemKind, Pat, PatId};
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
    Function,
    Struct,
    Enum,
    EnumVariant,
    Field,
    TypeParameter,
    Local,
    Module,
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
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
            by_file: HashMap::new(),
            by_location: HashMap::new(),
        }
    }

    pub fn build_from_hir(&mut self, file_id: FileId, hir: &CrateHir, interner: &Interner) {
        self.clear_file(file_id);

        for item in hir.items.iter() {
            let name = interner.resolve(item.name).to_string();
            let kind = match item.kind {
                ItemKind::Fn(_) => SymbolKind::Function,
                ItemKind::Struct(_) => SymbolKind::Struct,
                ItemKind::Enum(_) => SymbolKind::Enum,
                _ => continue,
            };
            let span = item.span;
            let def_loc = DefinitionLocation { file_id, span };
            let type_sig = match &item.kind {
                ItemKind::Fn(fn_item) => {
                    let params: Vec<(String, String)> = fn_item
                        .params
                        .iter()
                        .map(|p| {
                            let ty_str =
                                p.ty.as_ref()
                                    .map(|t| format!("{:?}", t))
                                    .unwrap_or_else(|| "unknown".to_string());
                            (interner.resolve(p.name).to_string(), ty_str)
                        })
                        .collect();
                    let return_ty = fn_item.return_ty.as_ref().map(|t| format!("{:?}", t));
                    Some(TypeSignature {
                        params,
                        return_type: return_ty,
                    })
                }
                ItemKind::Struct(struct_item) => {
                    let fields: Vec<(String, String)> = struct_item
                        .fields
                        .iter()
                        .map(|f| {
                            let ty_str = format!("{:?}", f.ty);
                            (interner.resolve(f.name).to_string(), ty_str)
                        })
                        .collect();
                    Some(TypeSignature {
                        params: fields,
                        return_type: None,
                    })
                }
                ItemKind::Enum(enum_item) => {
                    let variants: Vec<(String, String)> = enum_item
                        .variants
                        .iter()
                        .map(|v| {
                            let fields_str = if v.fields.is_empty() {
                                String::new()
                            } else {
                                let tys: Vec<String> =
                                    v.fields.iter().map(|f| format!("{:?}", f.ty)).collect();
                                format!("({})", tys.join(", "))
                            };
                            (interner.resolve(v.name).to_string(), fields_str)
                        })
                        .collect();
                    Some(TypeSignature {
                        params: variants,
                        return_type: None,
                    })
                }
                _ => None,
            };
            let is_pub = matches!(item.visibility, glyim_core::Visibility::Public);
            let info = SymbolInfo {
                name: name.clone(),
                kind,
                definition: def_loc,
                type_signature: type_sig,
                is_pub,
                documentation: None,
            };
            self.insert_symbol(file_id, info);
        }

        for (body_id, body) in hir.bodies.iter_enumerated() {
            let owner = hir.body_owners[body_id];
            self.index_body(file_id, body, interner, owner);
        }
    }

    fn index_body(
        &mut self,
        file_id: FileId,
        body: &Body,
        interner: &Interner,
        _owner: LocalDefId,
    ) {
        for (pat_id, pat) in body.pats.iter_enumerated() {
            self.index_pattern(file_id, pat_id, pat, body, interner);
        }
    }

    #[allow(clippy::only_used_in_recursion)]
    fn index_pattern(
        &mut self,
        file_id: FileId,
        _pat_id: PatId,
        pat: &Pat,
        body: &Body,
        interner: &Interner,
    ) {
        match pat {
            Pat::Binding {
                name,
                mutability: _,
                subpattern,
            } => {
                let name_str = interner.resolve(*name).to_string();
                // HIR does not store spans for patterns; use DUMMY for now
                let span = Span::DUMMY;
                let def_loc = DefinitionLocation { file_id, span };
                let info = SymbolInfo {
                    name: name_str,
                    kind: SymbolKind::Local,
                    definition: def_loc,
                    type_signature: None,
                    is_pub: false,
                    documentation: None,
                };
                self.insert_symbol(file_id, info);
                if let Some(sub) = subpattern {
                    self.index_pattern(file_id, *sub, pat, body, interner);
                }
            }
            Pat::Struct {
                path: _,
                fields,
                rest: _,
            } => {
                for (_, field_pat) in fields {
                    self.index_pattern(file_id, *field_pat, pat, body, interner);
                }
            }
            Pat::Tuple(pats) | Pat::Or(pats) => {
                for p in pats {
                    self.index_pattern(file_id, *p, pat, body, interner);
                }
            }
            _ => {}
        }
    }

    fn insert_symbol(&mut self, file_id: FileId, info: SymbolInfo) {
        let name = info.name.clone();
        self.by_name.entry(name).or_default().push(info.clone());
        self.by_file.entry(file_id).or_default().push(info.clone());
        self.by_location
            .insert((file_id.to_raw(), info.definition.span.lo.to_usize()), info);
    }

    pub fn lookup_by_name(&self, name: &str) -> Vec<&SymbolInfo> {
        self.by_name
            .get(name)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn lookup_by_location(&self, file_id: FileId, offset: usize) -> Option<&SymbolInfo> {
        self.by_location.get(&(file_id.to_raw(), offset))
    }

    pub fn symbols_in_file(&self, file_id: FileId) -> Vec<&SymbolInfo> {
        self.by_file
            .get(&file_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
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
                self.by_location
                    .remove(&(file_id.to_raw(), sym.definition.span.lo.to_usize()));
            }
        }
    }

    #[doc(hidden)]
    pub fn insert_test_symbol(&mut self, file_id: FileId, sym: SymbolInfo) {
        self.insert_symbol(file_id, sym);
    }
}
