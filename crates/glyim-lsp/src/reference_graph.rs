use glyim_core::Interner;
use glyim_hir::{Body, CrateHir, Expr, ExprId, ItemKind};
use glyim_span::{FileId, Span};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Reference {
    pub file_id: FileId,
    pub span: Span,
    pub is_definition: bool,
    pub kind: ReferenceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Call,
    TypeReference,
    FieldAccess,
    Constructor,
    Pattern,
    Definition,
}

pub struct ReferenceGraph {
    references: HashMap<String, Vec<Reference>>,
}

impl Default for ReferenceGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceGraph {
    pub fn new() -> Self {
        Self {
            references: HashMap::new(),
        }
    }

    pub fn build_from_hir(&mut self, file_id: FileId, hir: &CrateHir, interner: &Interner) {
        // Remove stale references for this file
        self.references
            .retain(|_, refs| refs.iter().all(|r| r.file_id != file_id));

        let mut add_ref = |name: &str, span: Span, is_def: bool, kind: ReferenceKind| {
            self.references
                .entry(name.to_string())
                .or_default()
                .push(Reference {
                    file_id,
                    span,
                    is_definition: is_def,
                    kind,
                });
        };

        // Record definitions from items
        for item in hir.items.iter() {
            let name = interner.resolve(item.name).to_string();
            add_ref(&name, item.span, true, ReferenceKind::Definition);

            if let ItemKind::Fn(fn_item) = &item.kind {
                for param in &fn_item.params {
                    let param_name = interner.resolve(param.name).to_string();
                    add_ref(&param_name, param.span, true, ReferenceKind::Definition);
                }
            }
            if let ItemKind::Struct(struct_item) = &item.kind {
                for field in &struct_item.fields {
                    let field_name = interner.resolve(field.name).to_string();
                    add_ref(&field_name, field.span, true, ReferenceKind::Definition);
                }
            }
            if let ItemKind::Enum(enum_item) = &item.kind {
                for variant in &enum_item.variants {
                    let variant_name = interner.resolve(variant.name).to_string();
                    add_ref(&variant_name, variant.span, true, ReferenceKind::Definition);
                }
            }
        }

        // Walk bodies to find references
        fn walk_expr(
            expr_id: ExprId,
            body: &Body,
            interner: &Interner,
            file_id: FileId,
            add_ref: &mut impl FnMut(&str, Span, bool, ReferenceKind),
        ) {
            let expr = &body.exprs[expr_id];
            let span = body.expr_spans.get(expr_id).copied().unwrap_or(Span::DUMMY);
            match expr {
                Expr::Path(path) => {
                    if let Some(name) = path.as_name() {
                        let name_str = interner.resolve(name).to_string();
                        add_ref(&name_str, span, false, ReferenceKind::TypeReference);
                    }
                }
                Expr::Call { func, args: _ } => {
                    walk_expr(*func, body, interner, file_id, add_ref);
                    if let Expr::Path(path) = &body.exprs[*func] {
                        if let Some(name) = path.as_name() {
                            let name_str = interner.resolve(name).to_string();
                            add_ref(&name_str, span, false, ReferenceKind::Call);
                        }
                    }
                }
                Expr::MethodCall {
                    receiver, method, ..
                } => {
                    walk_expr(*receiver, body, interner, file_id, add_ref);
                    let method_str = interner.resolve(*method).to_string();
                    add_ref(&method_str, span, false, ReferenceKind::Call);
                }
                Expr::Field { receiver, field } => {
                    walk_expr(*receiver, body, interner, file_id, add_ref);
                    let field_str = interner.resolve(*field).to_string();
                    add_ref(&field_str, span, false, ReferenceKind::FieldAccess);
                }
                Expr::Binary { lhs, rhs, .. } => {
                    walk_expr(*lhs, body, interner, file_id, add_ref);
                    walk_expr(*rhs, body, interner, file_id, add_ref);
                }
                Expr::Unary { expr, .. } => walk_expr(*expr, body, interner, file_id, add_ref),
                Expr::Block { stmts, tail } => {
                    for stmt in stmts {
                        walk_expr(*stmt, body, interner, file_id, add_ref);
                    }
                    if let Some(tail_expr) = tail {
                        walk_expr(*tail_expr, body, interner, file_id, add_ref);
                    }
                }
                Expr::If {
                    cond,
                    then_branch,
                    else_branch,
                } => {
                    walk_expr(*cond, body, interner, file_id, add_ref);
                    walk_expr(*then_branch, body, interner, file_id, add_ref);
                    if let Some(else_expr) = else_branch {
                        walk_expr(*else_expr, body, interner, file_id, add_ref);
                    }
                }
                Expr::Match { scrutinee, arms } => {
                    walk_expr(*scrutinee, body, interner, file_id, add_ref);
                    for arm in arms {
                        if let Some(guard) = arm.guard {
                            walk_expr(guard, body, interner, file_id, add_ref);
                        }
                        walk_expr(arm.body, body, interner, file_id, add_ref);
                    }
                }
                Expr::Return { value } => {
                    if let Some(val) = value {
                        walk_expr(*val, body, interner, file_id, add_ref);
                    }
                }
                Expr::Assign { lhs, rhs } => {
                    walk_expr(*lhs, body, interner, file_id, add_ref);
                    walk_expr(*rhs, body, interner, file_id, add_ref);
                }
                Expr::Loop { body: loop_body } => {
                    walk_expr(*loop_body, body, interner, file_id, add_ref)
                }
                Expr::While {
                    cond,
                    body: loop_body,
                } => {
                    walk_expr(*cond, body, interner, file_id, add_ref);
                    walk_expr(*loop_body, body, interner, file_id, add_ref);
                }
                Expr::For {
                    pat: _,
                    iterable,
                    body: loop_body,
                } => {
                    walk_expr(*iterable, body, interner, file_id, add_ref);
                    walk_expr(*loop_body, body, interner, file_id, add_ref);
                }
                Expr::Struct { fields, spread, .. } => {
                    for (_, field_expr) in fields {
                        walk_expr(*field_expr, body, interner, file_id, add_ref);
                    }
                    if let Some(spread_expr) = spread {
                        walk_expr(*spread_expr, body, interner, file_id, add_ref);
                    }
                }
                Expr::Array(elems) | Expr::Tuple(elems) => {
                    for elem in elems {
                        walk_expr(*elem, body, interner, file_id, add_ref);
                    }
                }
                Expr::Closure {
                    body: closure_body, ..
                } => {
                    walk_expr(*closure_body, body, interner, file_id, add_ref);
                }
                Expr::Cast { expr, .. } | Expr::Ref { expr, .. } => {
                    walk_expr(*expr, body, interner, file_id, add_ref);
                }
                _ => {}
            }
        }

        // Iterate bodies
        for (_, body) in hir.bodies.iter_enumerated() {
            for (expr_id, _) in body.exprs.iter_enumerated() {
                walk_expr(expr_id, body, interner, file_id, &mut add_ref);
            }
        }
    }

    pub fn find_references(&self, symbol_name: &str) -> &[Reference] {
        self.references
            .get(symbol_name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    #[doc(hidden)]
    pub fn insert_test_reference(&mut self, name: &str, reference: Reference) {
        self.references
            .entry(name.to_string())
            .or_default()
            .push(reference);
    }
}
