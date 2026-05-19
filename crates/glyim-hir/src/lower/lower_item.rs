use std::collections::HashMap;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::interner::{Interner, Name};
use glyim_core::primitives::*;
use glyim_diag::GlyimDiagnostic;
use glyim_syntax::{SyntaxKind, SyntaxNode};

use crate::{Body, BodyId, EnumItem, Field, FnItem, Item, ItemId, ItemKind, Param, Pat, PatId, StructItem, Variant, Visibility};

use super::{
    first_ident_text, is_type_node, lower_expr::lower_block_to_expr, lower_type::lower_type_ref,
    next_local_def_id, node_span,
};

pub(crate) fn collect_struct_fields(node: &SyntaxNode, interner: &mut Interner) -> Option<(Name, Vec<Name>)> {
    let name_str = first_ident_text(node)?;
    let name = interner.intern(&name_str);
    let mut fields = Vec::new();
    let tokens: Vec<_> = node.children_with_tokens().collect();
    let mut i = 0;
    while i < tokens.len() {
        if let glyim_syntax::SyntaxElement::Token(t) = &tokens[i]
            && t.kind() == SyntaxKind::Ident
            && i + 2 < tokens.len()
            && let glyim_syntax::SyntaxElement::Token(col) = &tokens[i + 1]
            && col.kind() == SyntaxKind::Colon
        {
            fields.push(interner.intern(t.text()));
            i += 3;
            continue;
        }
        i += 1;
    }
    Some((name, fields))
}

pub(crate) fn lower_fn_def(
    node: &SyntaxNode,
    interner: &mut Interner,
    local_def_counter: &mut u32,
    item_id_counter: &mut u32,
    bodies: &mut IndexVec<BodyId, Body>,
    body_owners: &mut IndexVec<BodyId, LocalDefId>,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
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
        expr_spans: IndexVec::new(),
    };

    for child in node.children() {
        if child.kind() == SyntaxKind::ParamList {
            for param_node in child.children().filter(|c| c.kind() == SyntaxKind::Param) {
                let (p, pat_id) = lower_param(&param_node, interner, &mut body.pats);
                params.push(p);
                body.params.push(pat_id);
            }
        }
    }

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

    if let Some(block_node) = node.children().find(|c| c.kind() == SyntaxKind::Block) {
        tracing::debug!("Found Block node in FnDef, lowering to expr");
        lower_block_to_expr(
            &block_node,
            interner,
            &mut body.exprs,
            &mut body.pats,
            &mut body.expr_spans,
            diags,
            struct_field_map,
        );
    } else {
        tracing::debug!("FnDef without Block node treated as foreign function");
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

pub(crate) fn lower_param(
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

pub(crate) fn lower_struct_def(
    node: &SyntaxNode,
    interner: &mut Interner,
    _local_def_counter: &mut u32,
    item_id_counter: &mut u32,
) -> Option<Item> {
    let name_str = first_ident_text(node)?;
    let name = interner.intern(&name_str);
    let mut fields = Vec::new();
    let kind;
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

pub(crate) fn lower_enum_def(
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

pub(crate) fn lower_variant(node: &SyntaxNode, interner: &mut Interner) -> Option<Variant> {
    let vname_str = first_ident_text(node)?;
    let vname = interner.intern(&vname_str);
    let mut fields = Vec::new();
    let kind;
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
