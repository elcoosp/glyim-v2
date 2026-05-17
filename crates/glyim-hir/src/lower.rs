use crate::{
    Body, BodyId, ConstRef, CrateHir, EnumItem, Expr, ExprId, Field, FnItem, Item, ItemId,
    ItemKind, Literal, MatchArm, Param, Pat, PatId, Path as HirPath, PathSegment, StructItem,
    TypeRef, Variant, Visibility,
};
use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::interner::Interner;
use glyim_core::primitives::*;
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

// ---------- helpers ----------

pub(crate) fn first_ident_text(node: &SyntaxNode) -> Option<String> {
    for el in node.children_with_tokens() {
        if let glyim_syntax::SyntaxElement::Token(t) = el
            && t.kind() == SyntaxKind::Ident
        {
            return Some(t.text().to_string());
        }
    }
    None
}

pub(crate) fn is_type_node(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::PathType
            | SyntaxKind::RefType
            | SyntaxKind::FnType
            | SyntaxKind::DynType
            | SyntaxKind::SliceType
            | SyntaxKind::ArrayType
            | SyntaxKind::TupleType
            | SyntaxKind::NeverType
            | SyntaxKind::InferType
    )
}

pub(crate) fn is_expr_node(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::Block
            | SyntaxKind::BinaryExpr
            | SyntaxKind::IfExpr
            | SyntaxKind::PathExpr
            | SyntaxKind::LitExpr
            | SyntaxKind::CallExpr
            | SyntaxKind::MethodCallExpr
            | SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::UnaryExpr
            | SyntaxKind::RefExpr
            | SyntaxKind::MatchExpr
            | SyntaxKind::WhileExpr
            | SyntaxKind::LoopExpr
            | SyntaxKind::ForExpr
            | SyntaxKind::AssignExpr
            | SyntaxKind::BreakExpr
            | SyntaxKind::ContinueExpr
            | SyntaxKind::ReturnExpr
            | SyntaxKind::CastExpr
            | SyntaxKind::ClosureExpr
            | SyntaxKind::ArrayExpr
            | SyntaxKind::TupleExpr
            | SyntaxKind::StructExpr
            | SyntaxKind::RangeExpr
    )
}

// ---------- entry ----------

pub(crate) fn lower_crate(root: &SyntaxNode, interner: &mut Interner) -> CrateHir {
    let mut items = IndexVec::new();
    let mut bodies = IndexVec::new();
    let mut body_owners = IndexVec::new();
    let mut local_def_counter = 0u32;

    let mut item_id_counter = 0u32;

    for child in root.children() {
        match child.kind() {
            SyntaxKind::FnDef => {
                if let Some(item) = lower_fn_def(
                    &child,
                    interner,
                    &mut local_def_counter,
                    &mut item_id_counter,
                    &mut bodies,
                    &mut body_owners,
                ) {
                    items.push(item);
                }
            }
            SyntaxKind::StructDef => {
                if let Some(item) = lower_struct_def(
                    &child,
                    interner,
                    &mut local_def_counter,
                    &mut item_id_counter,
                ) {
                    items.push(item);
                }
            }
            SyntaxKind::EnumDef => {
                if let Some(item) = lower_enum_def(
                    &child,
                    interner,
                    &mut local_def_counter,
                    &mut item_id_counter,
                ) {
                    items.push(item);
                }
            }
            // Other item kinds (Trait, Impl, Mod, etc.) are not yet lowered.
            _ => {}
        }
    }

    CrateHir {
        items,
        bodies,
        body_owners,
    }
}

fn next_local_def_id(counter: &mut u32) -> LocalDefId {
    let id = *counter;
    *counter += 1;
    LocalDefId::from_raw(id)
}

// ---------- items ----------

fn lower_fn_def(
    node: &SyntaxNode,
    interner: &mut Interner,
    local_def_counter: &mut u32,
    item_id_counter: &mut u32,
    bodies: &mut IndexVec<BodyId, Body>,
    body_owners: &mut IndexVec<BodyId, LocalDefId>,
) -> Option<Item> {
    let name_str = first_ident_text(node)?;
    let name = interner.intern(&name_str);

    let mut params = Vec::new();
    let mut return_ty = None;
    let owner = next_local_def_id(local_def_counter);
    let mut body = Body {
        owner,
        exprs: IndexVec::new(),
        pats: IndexVec::new(),
        params: Vec::new(),
        span: node_span(node),
        expr_spans: IndexVec::new(), // Initialize empty
    };

    // ParamList
    for child in node.children() {
        if child.kind() == SyntaxKind::ParamList {
            for param_node in child.children().filter(|c| c.kind() == SyntaxKind::Param) {
                let (p, pat_id) = lower_param(&param_node, interner, &mut body.pats);
                params.push(p);
                body.params.push(pat_id);
            }
        }
    }

    // Return type: scan tokens for Arrow, then take next type node
    let mut arrow_seen = false;
    for el in node.children_with_tokens() {
        match el {
            glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::Arrow => {
                arrow_seen = true;
            }
            glyim_syntax::SyntaxElement::Node(n) if arrow_seen && is_type_node(&n) => {
                return_ty = lower_type_ref(&n, interner);
                arrow_seen = false;
            }
            _ => {}
        }
    }

    // Block
    if let Some(block_node) = node.children().find(|c| c.kind() == SyntaxKind::Block) {
        tracing::debug!("Found Block node in FnDef, lowering to expr");
        lower_block_to_expr(
            &block_node,
            interner,
            &mut body.exprs,
            &mut body.pats,
            &mut body.expr_spans,
        );
    } else {
        // FIXME: FnDef without Block node
        unimplemented!("FnDef without Block node")
    }

    let bid = bodies.push(body);
    body_owners.push(owner);

    let id = ItemId::from_raw(*item_id_counter);
    *item_id_counter += 1;
    Some(Item {
        id,
        name,
        kind: ItemKind::Fn(FnItem {
            params,
            return_ty,
            body: Some(bid),
            is_unsafe: false,
            is_async: false,
            generic_params: Vec::new(),
            where_clauses: Vec::new(),
        }),
        visibility: Visibility::Inherited,
        span: node_span(node),
    })
}

fn lower_param(
    node: &SyntaxNode,
    interner: &mut Interner,
    pats: &mut IndexVec<PatId, Pat>,
) -> (Param, PatId) {
    let name_text = first_ident_text(node).unwrap_or_else(|| "_".to_string());
    let name = interner.intern(&name_text);
    let ty = node
        .children()
        .find(is_type_node)
        .and_then(|n| lower_type_ref(&n, interner));
    let pat = if name_text == "_" {
        Pat::Wild
    } else {
        Pat::Binding {
            name,
            mutability: Mutability::Not,
            subpattern: None,
        }
    };
    let pat_id = pats.push(pat);
    (
        Param {
            name,
            ty,
            span: node_span(node),
        },
        pat_id,
    )
}

fn lower_struct_def(
    node: &SyntaxNode,
    interner: &mut Interner,
    _local_def_counter: &mut u32,
    item_id_counter: &mut u32,
) -> Option<Item> {
    let name_str = first_ident_text(node)?;
    let name = interner.intern(&name_str);

    let mut fields = Vec::new();
    let kind;

    // Scan tokens for field pattern: Ident Colon type
    let tokens: Vec<_> = node.children_with_tokens().collect();
    let mut i = 0;
    let mut has_fields = false;
    while i < tokens.len() {
        if let glyim_syntax::SyntaxElement::Token(t) = &tokens[i]
            && t.kind() == SyntaxKind::Ident
            && i + 2 < tokens.len()
            && let glyim_syntax::SyntaxElement::Token(col) = &tokens[i + 1]
            && col.kind() == SyntaxKind::Colon
            && let glyim_syntax::SyntaxElement::Node(ty) = &tokens[i + 2]
            && is_type_node(ty)
        {
            let fname = interner.intern(t.text());
            let fty = lower_type_ref(ty, interner)?;
            fields.push(Field {
                name: fname,
                ty: fty,
                span: node_span(node),
            });
            has_fields = true;
            i += 3;
            continue;
        }
        i += 1;
    }

    // Check for tuple fields: if there's a TupleType node, it's a tuple struct
    if !has_fields {
        let mut tuple_types = Vec::new();
        for child in node.children() {
            if child.kind() == SyntaxKind::TupleType {
                for ty_node in child.children().filter(is_type_node) {
                    if let Some(fty) = lower_type_ref(&ty_node, interner) {
                        tuple_types.push(fty);
                    }
                }
            }
        }
        if !tuple_types.is_empty() {
            for fty in tuple_types {
                fields.push(Field {
                    name: interner.intern(""),
                    ty: fty,
                    span: node_span(node),
                });
            }
            kind = StructKind::Tuple;
        } else {
            kind = StructKind::Unit;
        }
    } else {
        kind = StructKind::Record;
    }

    let id = ItemId::from_raw(*item_id_counter);
    *item_id_counter += 1;
    Some(Item {
        id,
        name,
        kind: ItemKind::Struct(StructItem {
            fields,
            kind,
            generic_params: Vec::new(),
            where_clauses: Vec::new(),
        }),
        visibility: Visibility::Inherited,
        span: node_span(node),
    })
}

fn lower_enum_def(
    node: &SyntaxNode,
    interner: &mut Interner,
    _local_def_counter: &mut u32,
    item_id_counter: &mut u32,
) -> Option<Item> {
    let name_str = first_ident_text(node)?;
    let name = interner.intern(&name_str);

    let mut variants = Vec::new();
    if let Some(variant_list) = node
        .children()
        .find(|c| c.kind() == SyntaxKind::VariantList)
    {
        for vnode in variant_list
            .children()
            .filter(|c| c.kind() == SyntaxKind::EnumVariant)
        {
            if let Some(variant) = lower_variant(&vnode, interner) {
                variants.push(variant);
            }
        }
    }

    let id = ItemId::from_raw(*item_id_counter);
    *item_id_counter += 1;
    Some(Item {
        id,
        name,
        kind: ItemKind::Enum(EnumItem {
            variants,
            generic_params: Vec::new(),
            where_clauses: Vec::new(),
        }),
        visibility: Visibility::Inherited,
        span: node_span(node),
    })
}

fn lower_variant(node: &SyntaxNode, interner: &mut Interner) -> Option<Variant> {
    let vname_str = first_ident_text(node)?;
    let vname = interner.intern(&vname_str);

    let mut fields = Vec::new();
    let kind;

    // Check for tuple fields (LParen ... RParen)
    let mut in_paren = false;
    let mut has_tuple = false;
    for child in node.children_with_tokens() {
        match child {
            glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::LParen => {
                in_paren = true;
            }
            glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::RParen => {
                in_paren = false;
            }
            glyim_syntax::SyntaxElement::Node(n) if in_paren && is_type_node(&n) => {
                let fty = lower_type_ref(&n, interner)?;
                fields.push(Field {
                    name: interner.intern(""),
                    ty: fty,
                    span: node_span(node),
                });
                has_tuple = true;
            }
            _ => {}
        }
    }

    // Check for record fields (field list with StructField nodes)
    let mut has_record = false;
    for child in node.children() {
        if child.kind() == SyntaxKind::FieldList {
            for fnode in child
                .children()
                .filter(|c| c.kind() == SyntaxKind::StructField)
            {
                let fname_str = first_ident_text(&fnode).unwrap_or_default();
                let fname = interner.intern(&fname_str);
                let fty = fnode
                    .children()
                    .find(is_type_node)
                    .and_then(|n| lower_type_ref(&n, interner))?;
                fields.push(Field {
                    name: fname,
                    ty: fty,
                    span: node_span(node),
                });
                has_record = true;
            }
        }
    }

    if has_record {
        kind = StructKind::Record;
    } else if has_tuple {
        kind = StructKind::Tuple;
    } else {
        kind = StructKind::Unit;
    }

    Some(Variant {
        name: vname,
        fields,
        kind,
        span: node_span(node),
    })
}

// ---------- types ----------

pub(crate) fn lower_type_ref(node: &SyntaxNode, interner: &mut Interner) -> Option<TypeRef> {
    match node.kind() {
        SyntaxKind::PathType => {
            let path = lower_path_from_type(node, interner)?;
            Some(TypeRef::Path(path))
        }
        SyntaxKind::RefType => {
            let inner_node = node.children().find(is_type_node)?;
            let inner = lower_type_ref(&inner_node, interner)?;
            let mutability = if node.children_with_tokens().any(|c| {
                matches!(&c, glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::KwMut)
            }) { Mutability::Mut } else { Mutability::Not };
            Some(TypeRef::Ref {
                inner: Box::new(inner),
                mutability,
            })
        }
        SyntaxKind::FnType => {
            let mut params = Vec::new();
            let mut ret = None;
            let mut after_arrow = false;
            for child in node.children() {
                if child.kind() == SyntaxKind::Arrow {
                    after_arrow = true;
                    continue;
                }
                if is_type_node(&child) {
                    if let Some(ty) = lower_type_ref(&child, interner) {
                        if after_arrow {
                            ret = Some(Box::new(ty));
                        } else {
                            params.push(ty);
                        }
                    }
                }
            }
            Some(TypeRef::Fn { params, ret })
        }
        SyntaxKind::SliceType => {
            let inner = node
                .children()
                .find(is_type_node)
                .and_then(|n| lower_type_ref(&n, interner))?;
            Some(TypeRef::Slice(Box::new(inner)))
        }
        SyntaxKind::ArrayType => {
            // Array type contains inner type and length (ConstRef)
            let mut inner = None;
            let mut len = None;
            for child in node.children() {
                if is_type_node(&child) {
                    inner = lower_type_ref(&child, interner);
                } else if child.kind() == SyntaxKind::LitExpr {
                    // We can't extract length easily; stub with Error
                    len = Some(ConstRef::Error);
                }
            }
            let inner = inner?;
            Some(TypeRef::Array {
                inner: Box::new(inner),
                len: len.unwrap_or(ConstRef::Error),
            })
        }
        SyntaxKind::TupleType => {
            let mut elems = Vec::new();
            for child in node.children().filter(is_type_node) {
                if let Some(ty) = lower_type_ref(&child, interner) {
                    elems.push(ty);
                }
            }
            Some(TypeRef::Tuple(elems))
        }
        SyntaxKind::NeverType => Some(TypeRef::Never),
        SyntaxKind::InferType => Some(TypeRef::Infer),
        SyntaxKind::DynType => {
            let inner = node.children().find(|c| is_type_node(c));
            if let Some(ty_node) = inner {
                lower_type_ref(&ty_node, interner)
            } else {
                let path = lower_path_from_type(node, interner)?;
                Some(TypeRef::Path(path))
            }
        }
        _ => {
            tracing::warn!("STUB: unhandled type node {:?}", node.kind());
            None
        }
    }
}

fn lower_path_from_type(node: &SyntaxNode, interner: &mut Interner) -> Option<HirPath> {
    let mut segments = Vec::new();
    for el in node.children_with_tokens() {
        match el {
            glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::Ident => {
                segments.push(PathSegment {
                    name: interner.intern(t.text()),
                    generic_args: None,
                });
            }
            glyim_syntax::SyntaxElement::Node(n) if n.kind() == SyntaxKind::UsePath => {
                for t in n.children_with_tokens() {
                    if let glyim_syntax::SyntaxElement::Token(tt) = t
                        && tt.kind() == SyntaxKind::Ident
                    {
                        segments.push(PathSegment {
                            name: interner.intern(tt.text()),
                            generic_args: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    if segments.is_empty() {
        None
    } else {
        Some(HirPath {
            segments,
            kind: glyim_core::path::PathKind::Plain,
        })
    }
}

// ---------- expressions ----------

fn lower_block_to_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> ExprId {
    let mut stmts = Vec::new();
    let mut pending: Option<ExprId> = None;
    let mut last_has_semi = false;

    for child in node.children() {
        match child.kind() {
            SyntaxKind::ExprStmt => {
                let has_semi = child.children_with_tokens().any(|el| {
                    matches!(el, glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::Semicolon)
                });
                let mut chain_base: Option<ExprId> = None;
                for inner in child.children() {
                    if !is_expr_node(&inner) && inner.kind() != SyntaxKind::Block {
                        continue;
                    }
                    // Handle chained field/method calls (sibling nodes from parser)
                    if (inner.kind() == SyntaxKind::FieldExpr
                        || inner.kind() == SyntaxKind::MethodCallExpr)
                        && let Some(base_id) = chain_base
                    {
                        if let Some(id) = lower_field_or_method_with_receiver(
                            &inner, base_id, interner, exprs, pats, expr_spans,
                        ) {
                            chain_base = Some(id);
                        }
                        continue;
                    }
                    let current = lower_expr(&inner, interner, exprs, pats, expr_spans);
                    if let Some(id) = current {
                        if let Some(prev) = chain_base.take() {
                            stmts.push(prev);
                        } else if let Some(prev) = pending.take() {
                            stmts.push(prev);
                        }
                        chain_base = Some(id);
                    }
                }
                if let Some(base_id) = chain_base.take() {
                    pending = Some(base_id);
                    last_has_semi = has_semi;
                }
            }
            SyntaxKind::LetStmt => {
                let rhs_expr = child.children().find(is_expr_node);
                if let Some(rhs) = rhs_expr {
                    if let Some(prev) = pending.take() {
                        stmts.push(prev);
                    }
                    pending = lower_expr(&rhs, interner, exprs, pats, expr_spans);
                    last_has_semi = true;
                }
            }
            _ => {}
        }
    }

    let tail = if last_has_semi {
        if let Some(last) = pending.take() {
            stmts.push(last);
        }
        None
    } else {
        pending.take()
    };

    let expr = Expr::Block { stmts, tail };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    eid
}

/// Lower a FieldExpr or MethodCallExpr with a known receiver.
fn lower_field_or_method_with_receiver(
    node: &SyntaxNode,
    receiver_id: ExprId,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let mut found_dot = false;
    let mut name = None;
    for el in node.children_with_tokens() {
        match el {
            glyim_syntax::SyntaxElement::Token(ref t) if t.kind() == SyntaxKind::Dot => {
                found_dot = true;
            }
            glyim_syntax::SyntaxElement::Token(ref t)
                if found_dot && t.kind() == SyntaxKind::Ident =>
            {
                name = Some(interner.intern(t.text()));
                break;
            }
            _ => {}
        }
    }
    let name = name?;
    let is_method = node.children_with_tokens().any(
        |el| matches!(el, glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::LParen),
    );
    if is_method {
        let mut arg_ids = Vec::new();
        for child in node.children() {
            if (is_expr_node(&child) || child.kind() == SyntaxKind::Block)
                && let Some(id) = lower_expr(&child, interner, exprs, pats, expr_spans)
            {
                arg_ids.push(id);
            }
        }
        let expr = Expr::MethodCall {
            receiver: receiver_id,
            method: name,
            args: arg_ids,
        };
        let eid = exprs.push(expr);
        expr_spans.push(node_span(node));
        Some(eid)
    } else {
        let expr = Expr::Field {
            receiver: receiver_id,
            field: name,
        };
        let eid = exprs.push(expr);
        expr_spans.push(node_span(node));
        Some(eid)
    }
}

pub(crate) fn lower_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    match node.kind() {
        SyntaxKind::Block => Some(lower_block_to_expr(node, interner, exprs, pats, expr_spans)),
        SyntaxKind::BinaryExpr => lower_binary_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::IfExpr => lower_if_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::PathExpr => lower_path_expr(node, interner, exprs, expr_spans),
        SyntaxKind::LitExpr => lower_lit_expr(node, interner, exprs, expr_spans),
        SyntaxKind::CallExpr => lower_call_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::MethodCallExpr => {
            lower_method_call_expr(node, interner, exprs, pats, expr_spans)
        }
        SyntaxKind::UnaryExpr => lower_unary_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::RefExpr => lower_ref_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::MatchExpr => lower_match_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::WhileExpr => lower_while_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::LoopExpr => lower_loop_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::ForExpr => lower_for_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::AssignExpr => lower_assign_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::BreakExpr => lower_break_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::ContinueExpr => {
            let expr = Expr::Continue;
            let eid = exprs.push(expr);
            expr_spans.push(node_span(node));
            Some(eid)
        }
        SyntaxKind::CastExpr => lower_cast_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::FieldExpr => lower_field_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::IndexExpr => lower_index_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::ArrayExpr => lower_array_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::TupleExpr => lower_tuple_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::RangeExpr => lower_range_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::ReturnExpr => lower_return_expr(node, interner, exprs, pats, expr_spans),
        SyntaxKind::ClosureExpr => {
            let mut params = Vec::new();
            let mut body = None;
            for child in node.children() {
                match child.kind() {
                    SyntaxKind::ParamList => {
                        for param_node in child.children().filter(|c| c.kind() == SyntaxKind::Param) {
                            let (_, pat_id) = lower_param(&param_node, interner, pats);
                            params.push(pat_id);
                        }
                    }
                    _ if is_expr_node(&child) || child.kind() == SyntaxKind::Block => {
                        if body.is_none() {
                            body = lower_expr(&child, interner, exprs, pats, expr_spans);
                        }
                    }
                    _ => {}
                }
            }
            let body_id = body.unwrap_or_else(|| {
                let missing = Expr::Missing;
                exprs.push(missing)
            });
            let expr = Expr::Closure { params, body: body_id };
            let eid = exprs.push(expr);
            expr_spans.push(node_span(node));
            Some(eid)
        }
        SyntaxKind::StructExpr => {
            let mut path = None;
            let mut fields = Vec::new();
            let mut spread = None;
            let mut in_braces = false;
            for child in node.children() {
                if child.kind() == SyntaxKind::PathExpr || child.kind() == SyntaxKind::UsePath {
                    if path.is_none() {
                        path = lower_path_expr(&child, interner, exprs, expr_spans);
                    }
                } else if child.kind() == SyntaxKind::LBrace {
                    in_braces = true;
                } else if child.kind() == SyntaxKind::RBrace {
                    in_braces = false;
                } else if in_braces {
                    if child.kind() == SyntaxKind::DotDot && spread.is_none() {
                        let mut found_expr = false;
                        for next in node.children() {
                            if found_expr {
                                if let Some(spread_id) = lower_expr(&next, interner, exprs, pats, expr_spans) {
                                    spread = Some(spread_id);
                                }
                                break;
                            }
                            if next == child {
                                found_expr = true;
                            }
                        }
                    } else if child.kind() == SyntaxKind::StructField {
                        let field_name = first_ident_text(&child).unwrap_or_default();
                        let name = interner.intern(&field_name);
                        let expr_node = child.children().find(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block);
                        if let Some(expr_id) = expr_node.and_then(|n| lower_expr(&n, interner, exprs, pats, expr_spans)) {
                            fields.push((name, expr_id));
                        }
                    } else if is_expr_node(&child) && child.kind() != SyntaxKind::StructField {
                        // shorthand field name (e.g., "x" without colon)
                        let name = interner.intern(&child.text().to_string().trim());
                        let expr_id = lower_expr(&child, interner, exprs, pats, expr_spans);
                        if let Some(eid) = expr_id {
                            fields.push((name, eid));
                        }
                    }
                }
            }
            let path_id = path.unwrap_or_else(|| {
                let missing = Expr::Missing;
                exprs.push(missing)
            });
            let path_struct = if let Expr::Path(p) = &exprs[path_id] {
                p.clone()
            } else {
                HirPath {
                    segments: vec![],
                    kind: glyim_core::path::PathKind::Plain,
                }
            };
            let expr = Expr::Struct { path: path_struct, fields, spread };
            let eid = exprs.push(expr);
            expr_spans.push(node_span(node));
            Some(eid)
        }
        _ => {
            tracing::warn!("STUB: unknown expr {:?}", node.kind());
            None
        }
    }
}

// ---------- sub-expressions ----------

fn lower_binary_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    // First, try to find operator and adjacent expressions by token position
    let op_token = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| {
            !t.kind().is_trivia()
                && t.kind() != SyntaxKind::Ident
                && t.kind() != SyntaxKind::LParen
                && t.kind() != SyntaxKind::RParen
        });

    if let Some(op_token) = op_token {
        // Find LHS: expression node that ends before the operator starts
        let lhs_node = node
            .children_with_tokens()
            .take_while(|el| match el {
                glyim_syntax::SyntaxElement::Token(t) => t != &op_token,
                _ => true,
            })
            .filter_map(|el| el.as_node().cloned())
            .last()
            .filter(|n| is_expr_node(n) || n.kind() == SyntaxKind::Block);

        // Find RHS: expression node that starts after the operator ends
        let rhs_node = node
            .children_with_tokens()
            .skip_while(|el| match el {
                glyim_syntax::SyntaxElement::Token(t) => t != &op_token,
                _ => true,
            })
            .skip(1)
            .find_map(|el| el.as_node().cloned())
            .filter(|n| is_expr_node(n) || n.kind() == SyntaxKind::Block);

        if let (Some(lhs), Some(rhs)) = (lhs_node, rhs_node) {
            let lhs_id = lower_expr(&lhs, interner, exprs, pats, expr_spans)?;
            let rhs_id = lower_expr(&rhs, interner, exprs, pats, expr_spans)?;
            let op = lower_bin_op_token(&op_token);
            let expr = Expr::Binary {
                op,
                lhs: lhs_id,
                rhs: rhs_id,
            };
            let eid = exprs.push(expr);
            expr_spans.push(node_span(node));
            return Some(eid);
        }
    }

    // Fallback: collect all expression children in order
    let expr_children: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)
        .collect();

    if expr_children.len() < 2 {
        tracing::warn!("BinaryExpr with fewer than 2 expression children");
        return None;
    }

    let lhs_node = &expr_children[0];
    let rhs_node = &expr_children[1];
    let lhs_id = lower_expr(lhs_node, interner, exprs, pats, expr_spans)?;
    let rhs_id = lower_expr(rhs_node, interner, exprs, pats, expr_spans)?;
    // Find any operator token between them (for completeness)
    let lhs_range = lhs_node.text_range();
    let rhs_range = rhs_node.text_range();
    let op_token = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| {
            let range = t.text_range();
            range.start() >= lhs_range.end()
                && range.end() <= rhs_range.start()
                && !t.kind().is_trivia()
        });
    let op = op_token.map_or(BinOp::Add, |t| lower_bin_op_token(&t));
    let expr = Expr::Binary {
        op,
        lhs: lhs_id,
        rhs: rhs_id,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_bin_op_token(token: &SyntaxToken) -> BinOp {
    match token.text() {
        "+" => BinOp::Add,
        "-" => BinOp::Sub,
        "*" => BinOp::Mul,
        "/" => BinOp::Div,
        "%" => BinOp::Rem,
        "==" => BinOp::Eq,
        "!=" => BinOp::Ne,
        "<" => BinOp::Lt,
        ">" => BinOp::Gt,
        "<=" => BinOp::LtEq,
        ">=" => BinOp::GtEq,
        "&&" => BinOp::And,
        "||" => BinOp::Or,
        _ => {
            tracing::warn!("STUB: unknown bin op {:?}", token.text());
            BinOp::Add
        }
    }
}

fn lower_if_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let mut children: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)
        .collect();
    if children.len() < 2 {
        return None;
    }
    let cond = children.remove(0);
    let then_branch = children.remove(0);
    let else_branch = children.pop();
    let cond_id = lower_expr(&cond, interner, exprs, pats, expr_spans)?;
    let then_id = lower_expr(&then_branch, interner, exprs, pats, expr_spans)?;
    let else_id = else_branch.and_then(|e| lower_expr(&e, interner, exprs, pats, expr_spans));
    let expr = Expr::If {
        cond: cond_id,
        then_branch: then_id,
        else_branch: else_id,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_path_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let mut segments = Vec::new();
    for el in node.children_with_tokens() {
        match el {
            glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::Ident => {
                segments.push(PathSegment {
                    name: interner.intern(t.text()),
                    generic_args: None,
                });
            }
            glyim_syntax::SyntaxElement::Node(n) if n.kind() == SyntaxKind::UsePath => {
                for t in n.children_with_tokens() {
                    if let glyim_syntax::SyntaxElement::Token(tt) = t
                        && tt.kind() == SyntaxKind::Ident
                    {
                        segments.push(PathSegment {
                            name: interner.intern(tt.text()),
                            generic_args: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    if segments.is_empty() {
        return None;
    }
    let path = HirPath {
        segments,
        kind: glyim_core::path::PathKind::Plain,
    };
    let expr = Expr::Path(path);
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_lit_expr(
    node: &SyntaxNode,
    _interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let lit_token = node
        .children_with_tokens()
        .filter_map(|c| c.into_token())
        .find(|t| {
            t.kind().is_literal()
                || t.kind() == SyntaxKind::KwTrue
                || t.kind() == SyntaxKind::KwFalse
        })?;
    let lit = lower_literal(&lit_token);
    let expr = Expr::Literal(lit);
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

pub(crate) fn lower_literal(token: &SyntaxToken) -> Literal {
    let text = token.text().to_string();
    match token.kind() {
        SyntaxKind::IntLit => {
            // Strip suffix (e.g., 42i32 -> 42) and parse
            let (num_str, suffix) = split_int_literal(&text);
            let (value, is_unsigned) = parse_int_with_prefix(&num_str);
            if let Some(suffix) = suffix {
                match suffix.as_str() {
                    "i8" => return Literal::Int(value, Some(IntTy::I8)),
                    "i16" => return Literal::Int(value, Some(IntTy::I16)),
                    "i32" => return Literal::Int(value, Some(IntTy::I32)),
                    "i64" => return Literal::Int(value, Some(IntTy::I64)),
                    "isize" => return Literal::Int(value, Some(IntTy::Isize)),
                    "u8" => return Literal::Uint(value as u128, Some(UintTy::U8)),
                    "u16" => return Literal::Uint(value as u128, Some(UintTy::U16)),
                    "u32" => return Literal::Uint(value as u128, Some(UintTy::U32)),
                    "u64" => return Literal::Uint(value as u128, Some(UintTy::U64)),
                    "usize" => return Literal::Uint(value as u128, Some(UintTy::Usize)),
                    _ => {
                        tracing::warn!("Unknown integer suffix: {}", suffix);
                        return Literal::Int(value, None);
                    }
                }
            }
            if is_unsigned {
                Literal::Uint(value as u128, None)
            } else {
                Literal::Int(value, None)
            }
        }
        SyntaxKind::FloatLit => {
            let (num_str, _suffix) = split_float_literal(&text);
            if let Ok(f) = num_str.parse::<f64>() {
                return Literal::Float(f.to_bits(), FloatTy::F64);
            }
            tracing::warn!("Failed to parse float literal: {}", text);
            Literal::Unit
        }
        SyntaxKind::KwTrue | SyntaxKind::BoolLit if text == "true" => Literal::Bool(true),
        SyntaxKind::KwFalse | SyntaxKind::BoolLit if text == "false" => Literal::Bool(false),
        SyntaxKind::CharLit => {
            let inner = &text[1..text.len() - 1];
            if let Some(c) = parse_char_literal(inner) {
                Literal::Char(c)
            } else {
                Literal::Unit
            }
        }
        SyntaxKind::StringLit => {
            // For now, just create a dummy Name; real interner needed
            // String literals not fully supported in HIR lowering yet; return unit.
            Literal::Unit
        }
        _ => Literal::Unit,
    }
}

// Helper functions (to be added above)
fn split_int_literal(s: &str) -> (String, Option<String>) {
    // Find where the literal part ends (digits, hex, oct, bin) and suffix begins
    let mut i = 0;
    let chars: Vec<char> = s.chars().collect();
    // Skip leading '+' or '-'
    if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
        i += 1;
    }
    // Check for 0x, 0o, 0b
    if i + 1 < chars.len() && chars[i] == '0' {
        let prefix = chars[i+1];
        if prefix == 'x' || prefix == 'X' {
            i += 2;
            while i < chars.len() && (chars[i].is_ascii_hexdigit() || chars[i] == '_') {
                i += 1;
            }
        } else if prefix == 'o' || prefix == 'O' {
            i += 2;
            while i < chars.len() && (('0' <= chars[i] && chars[i] <= '7') || chars[i] == '_') {
                i += 1;
            }
        } else if prefix == 'b' || prefix == 'B' {
            i += 2;
            while i < chars.len() && (chars[i] == '0' || chars[i] == '1' || chars[i] == '_') {
                i += 1;
            }
        } else {
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_') {
                i += 1;
            }
        }
    } else {
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_') {
            i += 1;
        }
    }
    let num_part = &s[..i];
    let suffix = if i < s.len() { Some(&s[i..]) } else { None };
    (num_part.replace('_', ""), suffix.map(|s| s.to_string()))
}

fn parse_int_with_prefix(s: &str) -> (i128, bool) {
    let s = s.trim_start_matches('+');
    if s.starts_with("0x") || s.starts_with("0X") {
        let hex = &s[2..];
        let val = i128::from_str_radix(hex, 16).unwrap_or(0);
        (val, false)
    } else if s.starts_with("0o") || s.starts_with("0O") {
        let oct = &s[2..];
        let val = i128::from_str_radix(oct, 8).unwrap_or(0);
        (val, false)
    } else if s.starts_with("0b") || s.starts_with("0B") {
        let bin = &s[2..];
        let val = i128::from_str_radix(bin, 2).unwrap_or(0);
        (val, false)
    } else {
        let val = s.parse::<i128>().unwrap_or(0);
        (val, s.starts_with('-'))
    }
}

fn split_float_literal(s: &str) -> (String, Option<String>) {
    // Similar but simpler: find first non-digit, non-'.', non-'e', non-'E', non-'+', non'-'
    let mut digits_end = s.len();
    for (i, ch) in s.char_indices() {
        if !(ch.is_ascii_digit() || ch == '.' || ch == 'e' || ch == 'E' || ch == '+' || ch == '-') {
            digits_end = i;
            break;
        }
    }
    let num_part = &s[..digits_end];
    let suffix = if digits_end < s.len() {
        Some(&s[digits_end..])
    } else {
        None
    };
    (num_part.to_string(), suffix.map(|s| s.to_string()))
}

fn parse_char_literal(s: &str) -> Option<char> {
    if s.len() == 1 {
        return s.chars().next();
    }
    // Basic escape handling
    if let Some(stripped) = s.strip_prefix('\\') {
        match stripped {
            "n" => Some('\n'),
            "r" => Some('\r'),
            "t" => Some('\t'),
            "\\" => Some('\\'),
            "'" => Some('\''),
            "\"" => Some('\"'),
            _ => None,
        }
    } else {
        None
    }
}

fn lower_call_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let children: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_expr_node(c) || c.kind() == SyntaxKind::PathExpr)
        .collect();
    let func = children.first()?.clone();
    let args: Vec<SyntaxNode> = children.into_iter().skip(1).collect();
    let func_id = lower_expr(&func, interner, exprs, pats, expr_spans)?;
    let mut arg_ids = Vec::new();
    for arg in args {
        if let Some(id) = lower_expr(&arg, interner, exprs, pats, expr_spans) {
            arg_ids.push(id);
        }
    }
    let expr = Expr::Call {
        func: func_id,
        args: arg_ids,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_method_call_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    // MethodCallExpr structure:
    // PathExpr (receiver), Dot (token), Ident (method name), LParen, args..., RParen
    let receiver = node.children().find(|c| c.kind() == SyntaxKind::PathExpr)?;
    let receiver_id = lower_expr(&receiver, interner, exprs, pats, expr_spans)?;

    // Find the Ident token that comes directly after a Dot token
    let mut found_dot = false;
    let mut method_name = None;
    for el in node.children_with_tokens() {
        match el {
            glyim_syntax::SyntaxElement::Token(ref t) if t.kind() == SyntaxKind::Dot => {
                found_dot = true;
            }
            glyim_syntax::SyntaxElement::Token(ref t)
                if found_dot && t.kind() == SyntaxKind::Ident =>
            {
                method_name = Some(interner.intern(t.text()));
                break;
            }
            _ => {}
        }
    }
    let method = method_name?;

    // Args: expression nodes (not PathExpr which is receiver)
    let mut arg_ids = Vec::new();
    for child in node.children() {
        if child.kind() != SyntaxKind::PathExpr
            && (is_expr_node(&child) || child.kind() == SyntaxKind::Block)
            && let Some(id) = lower_expr(&child, interner, exprs, pats, expr_spans)
        {
            arg_ids.push(id);
        }
    }
    let expr = Expr::MethodCall {
        receiver: receiver_id,
        method,
        args: arg_ids,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_unary_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let op_token = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| {
            matches!(
                t.kind(),
                SyntaxKind::Bang | SyntaxKind::Minus | SyntaxKind::Star | SyntaxKind::And
            )
        })?;
    let inner = node
        .children()
        .find(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)?;
    let expr_id = lower_expr(&inner, interner, exprs, pats, expr_spans)?;
    // Check if this is a reference (&) operator -> produce Expr::Ref
    if op_token.kind() == SyntaxKind::And {
        let mutability = Mutability::Not; // &mut is handled by parser differently
        let expr = Expr::Ref {
            expr: expr_id,
            mutability,
        };
        let eid = exprs.push(expr);
        expr_spans.push(node_span(node));
        return Some(eid);
    }
    let op = match op_token.kind() {
        SyntaxKind::Bang => UnOp::Not,
        SyntaxKind::Minus => UnOp::Neg,
        SyntaxKind::Star => UnOp::Deref,
        _ => return None,
    };
    let expr = Expr::Unary { op, expr: expr_id };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_ref_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let inner = node
        .children()
        .find(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)?;
    let expr_id = lower_expr(&inner, interner, exprs, pats, expr_spans)?;
    let mutability = if node.children_with_tokens().any(
        |c| matches!(&c, glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::KwMut),
    ) {
        Mutability::Mut
    } else {
        Mutability::Not
    };
    let expr = Expr::Ref {
        expr: expr_id,
        mutability,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_match_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    // MatchExpr children: expression (scrutinee), MatchArmList
    let scrutinee = node
        .children()
        .find(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)?;
    let scrutinee_id = lower_expr(&scrutinee, interner, exprs, pats, expr_spans)?;
    let mut arms = Vec::new();
    if let Some(arm_list) = node
        .children()
        .find(|c| c.kind() == SyntaxKind::MatchArmList)
    {
        for arm_node in arm_list
            .children()
            .filter(|c| c.kind() == SyntaxKind::MatchArm)
        {
            let mut pat_id = None;
            let mut guard = None;
            let mut body_id = None;
            for part in arm_node.children() {
                match part.kind() {
                    SyntaxKind::PatIdent
                    | SyntaxKind::PatWild
                    | SyntaxKind::PatLit
                    | SyntaxKind::PatTuple
                    | SyntaxKind::PatStruct
                    | SyntaxKind::PatOr => {
                        pat_id = lower_pat(&part, interner, pats);
                    }
                    _ if is_expr_node(&part) => {
                        if body_id.is_none() {
                            body_id = lower_expr(&part, interner, exprs, pats, expr_spans);
                        } else if guard.is_none() {
                            guard = lower_expr(&part, interner, exprs, pats, expr_spans);
                        }
                    }
                    _ => {}
                }
            }
            if let (Some(pat), Some(body)) = (pat_id, body_id) {
                arms.push(MatchArm { pat, guard, body });
            }
        }
    }
    let expr = Expr::Match {
        scrutinee: scrutinee_id,
        arms,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

#[allow(unused_assignments)]
pub(crate) fn lower_pat(
    node: &SyntaxNode,
    interner: &mut Interner,
    pats: &mut IndexVec<PatId, Pat>,
) -> Option<PatId> {
    match node.kind() {
        SyntaxKind::PatIdent => {
            let name_text = first_ident_text(node).unwrap_or_else(|| "_".to_string());
            let name = interner.intern(&name_text);
            // Heuristic: if the name starts with an uppercase letter, treat as a path pattern;
            // otherwise, treat as a binding. This compensates for lack of name resolution in early lowering.
            if name_text.starts_with(|c: char| c.is_uppercase()) {
                let path = HirPath {
                    segments: vec![PathSegment {
                        name,
                        generic_args: None,
                    }],
                    kind: glyim_core::path::PathKind::Plain,
                };
                Some(pats.push(Pat::Path(path)))
            } else {
                // Look for subpattern (e.g., x @ 0..=5)
                let subpat = node
                    .children()
                    .find(|c| {
                        matches!(
                            c.kind(),
                            SyntaxKind::PatIdent
                                | SyntaxKind::PatWild
                                | SyntaxKind::PatLit
                                | SyntaxKind::PatTuple
                                | SyntaxKind::PatStruct
                                | SyntaxKind::PatOr
                        )
                    })
                    .and_then(|n| lower_pat(&n, interner, pats));
                Some(pats.push(Pat::Binding {
                    name,
                    mutability: Mutability::Not,
                    subpattern: subpat,
                }))
            }
        }
        SyntaxKind::PatWild => Some(pats.push(Pat::Wild)),
        SyntaxKind::PatLit => {
            // The parser emits a PatLit node containing either:
            //   - A single literal token (plain literal pattern)
            //   - Literal token, DotDot/DotDotEq, nested PatLit (range pattern)
            let children: Vec<glyim_syntax::SyntaxElement> = node.children_with_tokens().collect();
            // Check if this is a range pattern (has DotDot or DotDotEq)
            let is_range = children.iter().any(|c| {
                matches!(c, glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::DotDot || t.kind() == SyntaxKind::DotDotEq)
            });
            if is_range {
                let inclusive = children.iter().any(|c| {
                    matches!(c, glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::DotDotEq)
                });
                let mut start = None;
                let mut end = None;
                let mut before_dot = true;
                for child in &children {
                    match child {
                        glyim_syntax::SyntaxElement::Token(t)
                            if t.kind().is_literal()
                                || t.kind() == SyntaxKind::KwTrue
                                || t.kind() == SyntaxKind::KwFalse =>
                        {
                            let lit = lower_literal(t);
                            if before_dot {
                                start = Some(lit);
                            } else {
                                end = Some(lit);
                            }
                        }
                        glyim_syntax::SyntaxElement::Token(t)
                            if t.kind() == SyntaxKind::DotDot
                                || t.kind() == SyntaxKind::DotDotEq =>
                        {
                            before_dot = false;
                        }
                        glyim_syntax::SyntaxElement::Node(n) if n.kind() == SyntaxKind::PatLit => {
                            // Nested PatLit contains the end literal
                            if let Some(inner_lit) = lower_pat(n, interner, pats) {
                                // Extract literal from the inner pat
                                if let Pat::Literal(lit) = &pats[inner_lit] {
                                    end = Some(lit.clone());
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Some(pats.push(Pat::Range {
                    start,
                    end,
                    inclusive,
                }))
            } else {
                // Plain literal
                let lit_token = node
                    .children_with_tokens()
                    .filter_map(|c| c.into_token())
                    .find(|t| {
                        t.kind().is_literal()
                            || t.kind() == SyntaxKind::KwTrue
                            || t.kind() == SyntaxKind::KwFalse
                    })?;
                let lit = lower_literal(&lit_token);
                Some(pats.push(Pat::Literal(lit)))
            }
        }
        SyntaxKind::PatTuple => {
            // Check if this PatTuple is preceded by a UsePath sibling (e.g., Color(r,g,b) in let Color(r,g,b) = ...)
            if let Some(parent) = node.parent() {
                let siblings: Vec<glyim_syntax::SyntaxElement> =
                    parent.children_with_tokens().collect();
                let preceding_use_path = siblings
                    .iter()
                    .take_while(
                        |el| !matches!(el, glyim_syntax::SyntaxElement::Node(n) if *n == *node),
                    )
                    .filter_map(|el| {
                        if let glyim_syntax::SyntaxElement::Node(n) = el {
                            if n.kind() == SyntaxKind::UsePath {
                                Some(n.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .last();
                if let Some(use_path) = preceding_use_path {
                    let path_text = use_path.text().to_string().trim().to_string();
                    let path = HirPath {
                        segments: vec![PathSegment {
                            name: interner.intern(&path_text),
                            generic_args: None,
                        }],
                        kind: glyim_core::path::PathKind::Plain,
                    };
                    let mut fields = Vec::new();
                    for child in node.children() {
                        if let Some(pat_id) = lower_pat(&child, interner, pats) {
                            let field_name = {
                                let s = child.text().to_string();
                                interner.intern(s.trim())
                            };
                            fields.push((field_name, pat_id));
                        }
                    }
                    return Some(pats.push(Pat::Struct {
                        path,
                        fields,
                        rest: false,
                    }));
                }
            }
            // Otherwise, process as a regular tuple or mixed UsePath+PatTuple inside
            let mut elems = Vec::new();
            let children: Vec<glyim_syntax::SyntaxNode> = node.children().collect();
            let mut i = 0;
            while i < children.len() {
                let child = &children[i];
                if child.kind() == SyntaxKind::UsePath {
                    let mut segments = Vec::new();
                    for el in child.children_with_tokens() {
                        if let glyim_syntax::SyntaxElement::Token(t) = el
                            && t.kind() == SyntaxKind::Ident
                        {
                            segments.push(PathSegment {
                                name: interner.intern(t.text()),
                                generic_args: None,
                            });
                        }
                    }
                    let path = HirPath {
                        segments,
                        kind: glyim_core::path::PathKind::Plain,
                    };
                    let mut fields = Vec::new();
                    if i + 1 < children.len() && children[i + 1].kind() == SyntaxKind::PatTuple {
                        let args_node = &children[i + 1];
                        for el in args_node.children_with_tokens() {
                            match el {
                                glyim_syntax::SyntaxElement::Node(n)
                                    if matches!(
                                        n.kind(),
                                        SyntaxKind::PatIdent
                                            | SyntaxKind::PatWild
                                            | SyntaxKind::PatLit
                                            | SyntaxKind::PatTuple
                                            | SyntaxKind::PatStruct
                                            | SyntaxKind::PatOr
                                            | SyntaxKind::UsePath
                                    ) =>
                                {
                                    let arg_pat_id = lower_pat(&n, interner, pats);
                                    if let Some(pid) = arg_pat_id {
                                        let field_name = {
                                            let s = n.text().to_string();
                                            interner.intern(s.trim())
                                        };
                                        fields.push((field_name, pid));
                                    }
                                }
                                _ => {}
                            }
                        }
                        i += 2;
                    } else {
                        i += 1;
                    }
                    let struct_pat = Pat::Struct {
                        path,
                        fields,
                        rest: false,
                    };
                    elems.push(pats.push(struct_pat));
                } else {
                    if let Some(pat_id) = lower_pat(child, interner, pats) {
                        elems.push(pat_id);
                    }
                    i += 1;
                }
            }
            Some(pats.push(Pat::Tuple(elems)))
        }
        SyntaxKind::PatOr => {
            let mut pat_ids = Vec::new();
            // Similar handling: UsePath + PatTuple inside PatOr creates PatStruct arms.
            let children: Vec<glyim_syntax::SyntaxNode> = node.children().collect();
            let mut i = 0;
            while i < children.len() {
                let child = &children[i];
                if child.kind() == SyntaxKind::UsePath {
                    // Struct-like pattern in or: Some(x)
                    let mut segments = Vec::new();
                    for el in child.children_with_tokens() {
                        if let glyim_syntax::SyntaxElement::Token(t) = el
                            && t.kind() == SyntaxKind::Ident
                        {
                            segments.push(PathSegment {
                                name: interner.intern(t.text()),
                                generic_args: None,
                            });
                        }
                    }
                    let path = HirPath {
                        segments,
                        kind: glyim_core::path::PathKind::Plain,
                    };
                    let mut fields = Vec::new();
                    if i + 1 < children.len() && children[i + 1].kind() == SyntaxKind::PatTuple {
                        let args_node = &children[i + 1];
                        for el in args_node.children_with_tokens() {
                            match el {
                                glyim_syntax::SyntaxElement::Node(n)
                                    if matches!(
                                        n.kind(),
                                        SyntaxKind::PatIdent
                                            | SyntaxKind::PatWild
                                            | SyntaxKind::PatLit
                                            | SyntaxKind::PatTuple
                                            | SyntaxKind::PatStruct
                                            | SyntaxKind::PatOr
                                            | SyntaxKind::UsePath
                                    ) =>
                                {
                                    if let Some(pid) = lower_pat(&n, interner, pats) {
                                        let field_name = {
                                            let s = n.text().to_string();
                                            interner.intern(s.trim())
                                        };
                                        fields.push((field_name, pid));
                                    }
                                }
                                _ => {}
                            }
                        }
                        i += 2;
                    } else {
                        i += 1;
                    }
                    let struct_pat = Pat::Struct {
                        path,
                        fields,
                        rest: false,
                    };
                    pat_ids.push(pats.push(struct_pat));
                } else {
                    if let Some(pat_id) = lower_pat(child, interner, pats) {
                        pat_ids.push(pat_id);
                    }
                    i += 1;
                }
            }
            Some(pats.push(Pat::Or(pat_ids)))
        }
        SyntaxKind::PatStruct => {
            // Extract path from the parent context (UsePath before PatStruct)
            // Actually, the parser gives us UsePath as a sibling, then PatStruct with LBrace...RBrace.
            // But in some cases (let Point { x, y: 0 } = ...), the parser gives PatStruct directly.
            // We need to look at the parent node for a UsePath sibling.
            // For now, try to extract the path from a preceding sibling in the parent.
            let parent = node.parent()?;
            let siblings: Vec<glyim_syntax::SyntaxElement> =
                parent.children_with_tokens().collect();
            // Find the preceding sibling that is a UsePath or PathExpr
            let path = 'path_lookup: {
                let mut found = None;
                for el in &siblings {
                    if let glyim_syntax::SyntaxElement::Node(n) = el {
                        if n.kind() == SyntaxKind::UsePath || n.kind() == SyntaxKind::PathExpr {
                            found = Some(n.clone());
                        }
                        if *n == *node {
                            break 'path_lookup found;
                        }
                    }
                }
                None
            };
            let path = match path {
                Some(use_path) => {
                    let text = use_path.text().to_string().trim().to_string();
                    let name = interner.intern(&text);
                    HirPath {
                        segments: vec![PathSegment {
                            name,
                            generic_args: None,
                        }],
                        kind: glyim_core::path::PathKind::Plain,
                    }
                }
                None => return None,
            };
            let mut fields = Vec::new();
            let mut rest = false;
            let children: Vec<glyim_syntax::SyntaxElement> = node.children_with_tokens().collect();
            let mut i = 0;
            while i < children.len() {
                match &children[i] {
                    glyim_syntax::SyntaxElement::Node(n) if n.kind() == SyntaxKind::PatIdent => {
                        // Field: either shorthand (no colon) or with subpattern after colon
                        let field_name_text = first_ident_text(n).unwrap_or_default();
                        let name = interner.intern(&field_name_text);
                        // Check next non-trivia token for colon
                        let mut j = i + 1;
                        let mut has_colon = false;
                        while j < children.len() {
                            if let glyim_syntax::SyntaxElement::Token(t) = &children[j] {
                                if t.kind() == SyntaxKind::Colon {
                                    has_colon = true;
                                    break;
                                } else if !t.kind().is_trivia() {
                                    break;
                                }
                            }
                            j += 1;
                        }
                        if has_colon {
                            // Find the pattern node after the colon
                            let mut k = j + 1;
                            while k < children.len() {
                                if let glyim_syntax::SyntaxElement::Node(sub_n) = &children[k]
                                    && matches!(
                                        sub_n.kind(),
                                        SyntaxKind::PatIdent
                                            | SyntaxKind::PatWild
                                            | SyntaxKind::PatLit
                                            | SyntaxKind::PatTuple
                                            | SyntaxKind::PatStruct
                                            | SyntaxKind::PatOr
                                    )
                                {
                                    if let Some(pat_id) = lower_pat(sub_n, interner, pats) {
                                        fields.push((name, pat_id));
                                    }
                                    break;
                                }
                                k += 1;
                            }
                            i = j + 1; // continue after the colon (the subpattern is consumed by lower_pat)
                        } else {
                            // Shorthand binding
                            let binding_id = pats.push(Pat::Binding {
                                name,
                                mutability: Mutability::Not,
                                subpattern: None,
                            });
                            fields.push((name, binding_id));
                            i += 1;
                        }
                    }
                    glyim_syntax::SyntaxElement::Token(t) => {
                        if t.kind() == SyntaxKind::DotDot {
                            rest = true;
                        }
                        i += 1;
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            Some(pats.push(Pat::Struct { path, fields, rest }))
        }
        SyntaxKind::UsePath => {
            // Standalone UsePath (e.g., `None` in match arm).
            // Extract path and create Pat::Path.
            let mut segments = Vec::new();
            for el in node.children_with_tokens() {
                if let glyim_syntax::SyntaxElement::Token(t) = el
                    && t.kind() == SyntaxKind::Ident
                {
                    segments.push(PathSegment {
                        name: interner.intern(t.text()),
                        generic_args: None,
                    });
                }
            }
            if segments.is_empty() {
                return None;
            }
            let path = HirPath {
                segments,
                kind: glyim_core::path::PathKind::Plain,
            };
            Some(pats.push(Pat::Path(path)))
        }
        SyntaxKind::PathExpr => {
            // Standalone PathExpr (rare in pattern context, but handle)
            let mut segments = Vec::new();
            for el in node.children_with_tokens() {
                if let glyim_syntax::SyntaxElement::Token(t) = el
                    && t.kind() == SyntaxKind::Ident
                {
                    segments.push(PathSegment {
                        name: interner.intern(t.text()),
                        generic_args: None,
                    });
                }
            }
            if segments.is_empty() {
                return None;
            }
            let path = HirPath {
                segments,
                kind: glyim_core::path::PathKind::Plain,
            };
            Some(pats.push(Pat::Path(path)))
        }
        _ => {
            tracing::warn!("STUB: unknown pattern kind {:?}", node.kind());
            None
        }
    }
}

fn lower_while_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let mut children: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)
        .collect();
    if children.len() < 2 {
        return None;
    }
    let cond = children.remove(0);
    let body = children.remove(0);
    let cond_id = lower_expr(&cond, interner, exprs, pats, expr_spans)?;
    let body_id = lower_expr(&body, interner, exprs, pats, expr_spans)?;
    let expr = Expr::While {
        cond: cond_id,
        body: body_id,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_loop_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let body = node.children().find(|c| c.kind() == SyntaxKind::Block)?;
    let body_id = lower_expr(&body, interner, exprs, pats, expr_spans)?;
    let expr = Expr::Loop { body: body_id };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_for_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    // ForExpr children: pattern (PatIdent/PatWild), expression (iterable), Block
    let mut children = node.children();
    let pat_node = children.find(|c| {
        matches!(
            c.kind(),
            SyntaxKind::PatIdent
                | SyntaxKind::PatWild
                | SyntaxKind::PatTuple
                | SyntaxKind::PatStruct
        )
    })?;
    let iterable_node = children.find(|c| is_expr_node(c) || c.kind() == SyntaxKind::RangeExpr)?;
    let body_node = children.find(|c| c.kind() == SyntaxKind::Block)?;
    let pat_id = lower_pat(&pat_node, interner, pats)?;
    let iterable_id = lower_expr(&iterable_node, interner, exprs, pats, expr_spans)?;
    let body_id = lower_expr(&body_node, interner, exprs, pats, expr_spans)?;
    let expr = Expr::For {
        pat: pat_id,
        iterable: iterable_id,
        body: body_id,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_assign_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let mut children: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_expr_node(c) || c.kind() == SyntaxKind::PathExpr)
        .collect();
    if children.len() < 2 {
        return None;
    }
    let lhs = children.remove(0);
    let rhs = children.remove(0);
    let lhs_id = lower_expr(&lhs, interner, exprs, pats, expr_spans)?;
    let rhs_id = lower_expr(&rhs, interner, exprs, pats, expr_spans)?;
    let expr = Expr::Assign {
        lhs: lhs_id,
        rhs: rhs_id,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_return_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let value = node
        .children()
        .find(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)
        .and_then(|n| lower_expr(&n, interner, exprs, pats, expr_spans));
    let expr = Expr::Return { value };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_break_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let value = node
        .children()
        .find(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)
        .and_then(|n| lower_expr(&n, interner, exprs, pats, expr_spans));
    let expr = Expr::Break { value };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_cast_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let mut expr_node = None;
    let mut type_node = None;
    for child in node.children() {
        if is_expr_node(&child) && expr_node.is_none() {
            expr_node = Some(child);
        } else if is_type_node(&child) {
            type_node = Some(child);
        }
    }
    let expr_id = lower_expr(&expr_node?, interner, exprs, pats, expr_spans)?;
    let ty = lower_type_ref(&type_node?, interner)?;
    let expr = Expr::Cast { expr: expr_id, ty };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_field_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let receiver = node.children().find(|c| c.kind() == SyntaxKind::PathExpr)?;
    let receiver_id = lower_expr(&receiver, interner, exprs, pats, expr_spans)?;
    // Field name is the Ident token after Dot
    let mut found_dot = false;
    let mut field_name = None;
    for el in node.children_with_tokens() {
        match el {
            glyim_syntax::SyntaxElement::Token(ref t) if t.kind() == SyntaxKind::Dot => {
                found_dot = true;
            }
            glyim_syntax::SyntaxElement::Token(ref t)
                if found_dot && t.kind() == SyntaxKind::Ident =>
            {
                field_name = Some(interner.intern(t.text()));
                break;
            }
            _ => {}
        }
    }
    let field = field_name?;
    let expr = Expr::Field {
        receiver: receiver_id,
        field,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_index_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let mut children: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)
        .collect();
    if children.len() < 2 {
        return None;
    }
    let base = children.remove(0);
    let index = children.remove(0);
    let base_id = lower_expr(&base, interner, exprs, pats, expr_spans)?;
    let index_id = lower_expr(&index, interner, exprs, pats, expr_spans)?;
    let expr = Expr::Index {
        base: base_id,
        index: index_id,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_array_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let mut elems = Vec::new();
    for child in node.children().filter(is_expr_node) {
        if let Some(id) = lower_expr(&child, interner, exprs, pats, expr_spans) {
            elems.push(id);
        }
    }
    let expr = Expr::Array(elems);
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_tuple_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let mut elems = Vec::new();
    for child in node.children().filter(is_expr_node) {
        if let Some(id) = lower_expr(&child, interner, exprs, pats, expr_spans) {
            elems.push(id);
        }
    }
    let expr = Expr::Tuple(elems);
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn lower_range_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    exprs: &mut IndexVec<ExprId, Expr>,
    pats: &mut IndexVec<PatId, Pat>,
    expr_spans: &mut IndexVec<ExprId, Span>,
) -> Option<ExprId> {
    let children: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_expr_node(c) || c.kind() == SyntaxKind::LitExpr)
        .collect();
    let start = children
        .first()
        .and_then(|n| lower_expr(n, interner, exprs, pats, expr_spans));
    let end = children
        .get(1)
        .and_then(|n| lower_expr(n, interner, exprs, pats, expr_spans));
    // inclusive if DotDotEq token present
    let inclusive = node.children_with_tokens().any(
        |c| matches!(&c, glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::DotDotEq),
    );
    let expr = Expr::Range {
        start,
        end,
        inclusive,
    };
    let eid = exprs.push(expr);
    expr_spans.push(node_span(node));
    Some(eid)
}

fn node_span(node: &SyntaxNode) -> Span {
    let range = node.text_range();
    let lo = ByteIdx::from_raw(u32::from(range.start()));
    let hi = ByteIdx::from_raw(u32::from(range.end()));
    Span::new(FileId::from_raw(1), lo, hi, SyntaxContext::ROOT)
}
