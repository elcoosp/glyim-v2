use glyim_core::Interner;
use glyim_hir::{Body, CrateHir, Expr, ExprId, ItemKind, Pat};
use glyim_span::{FileId, Span};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Reference {
    pub file_id: FileId,
    pub span: Span,
    pub is_definition: bool,
    pub kind: ReferenceKind,
    pub def_id: Option<glyim_core::def_id::DefId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    Call,
    TypeReference,
    FieldAccess,
    Constructor,
    Pattern,
    Definition,
    Variable,
}

pub struct ReferenceGraph {
    references: HashMap<String, Vec<Reference>>,
    function_names: HashSet<String>,
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
            function_names: HashSet::new(),
        }
    }

    pub fn build_from_hir(&mut self, file_id: FileId, hir: &CrateHir, interner: &Interner) {
        self.references
            .retain(|_, refs| refs.iter().all(|r| r.file_id != file_id));

        for item in hir.items.iter() {
            if let ItemKind::Fn(_) = &item.kind {
                let name = interner.resolve(item.name).to_string();
                self.function_names.insert(name);
            }
        }

        let mut seen = HashSet::new();
        let function_names = &self.function_names;

        let mut add_ref = |name: &str, span: Span, is_def: bool, kind: ReferenceKind| {
            let key = (
                name.to_string(),
                file_id,
                span.lo.to_usize(),
                span.hi.to_usize(),
                kind,
            );
            if seen.insert(key) {
                eprintln!("REF: {} is_def={:?} kind={:?}", name, is_def, kind);
                self.references
                    .entry(name.to_string())
                    .or_default()
                    .push(Reference {
                        file_id,
                        span,
                        is_definition: is_def,
                        kind,
                        def_id: None,
                    });
            }
        };

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

        fn walk_pattern(
            pat_id: glyim_hir::PatId,
            body: &Body,
            interner: &Interner,
            add_ref: &mut impl FnMut(&str, Span, bool, ReferenceKind),
        ) {
            let pat = &body.pats[pat_id];
            match pat {
                Pat::Binding { name, .. } => {
                    let name_str = interner.resolve(*name).to_string();
                    add_ref(&name_str, Span::DUMMY, true, ReferenceKind::Definition);
                }
                Pat::Struct { fields, .. } => {
                    for (_, pat_id) in fields {
                        walk_pattern(*pat_id, body, interner, add_ref);
                    }
                }
                Pat::Tuple(pats) | Pat::Or(pats) => {
                    for p in pats {
                        walk_pattern(*p, body, interner, add_ref);
                    }
                }
                _ => {}
            }
        }

        fn extract_path_name(expr_id: ExprId, body: &Body, interner: &Interner) -> Option<String> {
            let expr = &body.exprs[expr_id];
            match expr {
                Expr::Path(path) => path
                    .as_name()
                    .map(|name| interner.resolve(name).to_string()),
                Expr::Ref { expr, .. } => extract_path_name(*expr, body, interner),
                Expr::Unary { expr, .. } => extract_path_name(*expr, body, interner),
                Expr::Field { receiver, .. } => extract_path_name(*receiver, body, interner),
                _ => None,
            }
        }

        fn walk_expr(
            expr_id: ExprId,
            body: &Body,
            interner: &Interner,
            _file_id: FileId,
            add_ref: &mut impl FnMut(&str, Span, bool, ReferenceKind),
            function_names: &HashSet<String>,
            in_call_func: bool,
        ) {
            let expr = &body.exprs[expr_id];
            let span = body.expr_spans.get(expr_id).copied().unwrap_or(Span::DUMMY);

            match expr {
                Expr::Path(path) => {
                    if let Some(name) = path.as_name() {
                        let name_str = interner.resolve(name).to_string();
                        if !in_call_func && !function_names.contains(&name_str) {
                            eprintln!("PATH use: {}", name_str);
                            add_ref(&name_str, span, false, ReferenceKind::Variable);
                        }
                    }
                }
                Expr::Call { func, args } => {
                    if let Some(name) = extract_path_name(*func, body, interner) {
                        eprintln!("CALL function: {}", name);
                        add_ref(&name, span, false, ReferenceKind::Call);
                    }
                    walk_expr(
                        *func,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        true,
                    );
                    for arg in args {
                        walk_expr(
                            *arg,
                            body,
                            interner,
                            _file_id,
                            add_ref,
                            function_names,
                            false,
                        );
                    }
                }
                Expr::MethodCall {
                    receiver,
                    method,
                    args,
                    ..
                } => {
                    walk_expr(
                        *receiver,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                    let method_str = interner.resolve(*method).to_string();
                    eprintln!("METHOD call: {}", method_str);
                    add_ref(&method_str, span, false, ReferenceKind::Call);
                    for arg in args {
                        walk_expr(
                            *arg,
                            body,
                            interner,
                            _file_id,
                            add_ref,
                            function_names,
                            false,
                        );
                    }
                }
                Expr::Field { receiver, field } => {
                    walk_expr(
                        *receiver,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                    let field_str = interner.resolve(*field).to_string();
                    eprintln!("FIELD access: {}", field_str);
                    add_ref(&field_str, span, false, ReferenceKind::FieldAccess);
                }
                Expr::Binary { lhs, rhs, .. } => {
                    walk_expr(
                        *lhs,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                    walk_expr(
                        *rhs,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                }
                Expr::Unary { expr, .. } => {
                    walk_expr(
                        *expr,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                }
                Expr::Block { stmts, tail } => {
                    for stmt in stmts {
                        walk_expr(
                            *stmt,
                            body,
                            interner,
                            _file_id,
                            add_ref,
                            function_names,
                            false,
                        );
                    }
                    if let Some(tail_expr) = tail {
                        walk_expr(
                            *tail_expr,
                            body,
                            interner,
                            _file_id,
                            add_ref,
                            function_names,
                            false,
                        );
                    }
                }
                Expr::If {
                    cond,
                    then_branch,
                    else_branch,
                } => {
                    walk_expr(
                        *cond,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                    walk_expr(
                        *then_branch,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                    if let Some(else_expr) = else_branch {
                        walk_expr(
                            *else_expr,
                            body,
                            interner,
                            _file_id,
                            add_ref,
                            function_names,
                            false,
                        );
                    }
                }
                Expr::Match { scrutinee, arms } => {
                    walk_expr(
                        *scrutinee,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                    for arm in arms {
                        walk_pattern(arm.pat, body, interner, add_ref);
                        if let Some(guard) = arm.guard {
                            walk_expr(
                                guard,
                                body,
                                interner,
                                _file_id,
                                add_ref,
                                function_names,
                                false,
                            );
                        }
                        walk_expr(
                            arm.body,
                            body,
                            interner,
                            _file_id,
                            add_ref,
                            function_names,
                            false,
                        );
                    }
                }
                Expr::Return { value: Some(val) } => {
                    walk_expr(
                        *val,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                }
                Expr::Return { value: None } => {}
                Expr::Assign { lhs, rhs } => {
                    if let Expr::Path(path) = &body.exprs[*lhs]
                        && let Some(name) = path.as_name()
                    {
                        let name_str = interner.resolve(name).to_string();
                        eprintln!("ASSIGN LHS definition: {}", name_str);
                        add_ref(&name_str, span, true, ReferenceKind::Variable);
                    }
                    // Walk RHS only (not LHS to avoid duplicate use)
                    walk_expr(
                        *rhs,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                }
                Expr::Loop { body: loop_body } => {
                    walk_expr(
                        *loop_body,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                }
                Expr::While {
                    cond,
                    body: loop_body,
                } => {
                    walk_expr(
                        *cond,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                    walk_expr(
                        *loop_body,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                }
                Expr::For {
                    pat,
                    iterable,
                    body: loop_body,
                } => {
                    walk_pattern(*pat, body, interner, add_ref);
                    walk_expr(
                        *iterable,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                    walk_expr(
                        *loop_body,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                }
                Expr::Struct { fields, spread, .. } => {
                    for (_, field_expr) in fields {
                        walk_expr(
                            *field_expr,
                            body,
                            interner,
                            _file_id,
                            add_ref,
                            function_names,
                            false,
                        );
                    }
                    if let Some(spread_expr) = spread {
                        walk_expr(
                            *spread_expr,
                            body,
                            interner,
                            _file_id,
                            add_ref,
                            function_names,
                            false,
                        );
                    }
                }
                Expr::Array(elems) | Expr::Tuple(elems) => {
                    for elem in elems {
                        walk_expr(
                            *elem,
                            body,
                            interner,
                            _file_id,
                            add_ref,
                            function_names,
                            false,
                        );
                    }
                }
                Expr::Closure {
                    body: closure_body, ..
                } => {
                    walk_expr(
                        *closure_body,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                }
                Expr::Cast { expr, .. } | Expr::Ref { expr, .. } => {
                    walk_expr(
                        *expr,
                        body,
                        interner,
                        _file_id,
                        add_ref,
                        function_names,
                        false,
                    );
                }
                _ => {
                    // Fallback: if we don't handle a variant, we still might need to recurse.
                    // We'll just do nothing to avoid infinite recursion.
                }
            }
        }

        for (_, body) in hir.bodies.iter_enumerated() {
            for param in &body.params {
                walk_pattern(*param, body, interner, &mut add_ref);
            }
            for (expr_id, _) in body.exprs.iter_enumerated() {
                walk_expr(
                    expr_id,
                    body,
                    interner,
                    file_id,
                    &mut add_ref,
                    function_names,
                    false,
                );
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
