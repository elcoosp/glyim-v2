#[allow(unused_imports)]
pub(crate) mod lower_expr;
pub(crate) mod lower_item;
pub(crate) mod lower_pat;
pub(crate) mod lower_type;

#[cfg(test)]
pub(crate) use lower_expr::{lower_expr, lower_literal};
#[cfg(test)]
pub(crate) use lower_pat::lower_pat;
#[cfg(test)]
pub(crate) use lower_type::lower_type_ref;

use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::interner::Interner;
use glyim_diag::GlyimDiagnostic;
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_syntax::{SyntaxKind, SyntaxNode};

use crate::CrateHir;

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

pub(crate) fn node_span(node: &SyntaxNode) -> Span {
    let range = node.text_range();
    let lo = ByteIdx::from_raw(u32::from(range.start()));
    let hi = ByteIdx::from_raw(u32::from(range.end()));
    Span::new(FileId::from_raw(1), lo, hi, SyntaxContext::ROOT)
}

fn next_local_def_id(counter: &mut u32) -> LocalDefId {
    let id = *counter;
    *counter += 1;
    LocalDefId::from_raw(id)
}

// ---------- entry ----------

pub(crate) fn lower_crate(
    root: &SyntaxNode,
    interner: &mut Interner,
    diags: &mut Vec<GlyimDiagnostic>,
) -> CrateHir {
    let mut items = IndexVec::new();
    let mut bodies = IndexVec::new();
    let mut body_owners = IndexVec::new();
    let mut local_def_counter = 0u32;
    let mut item_id_counter = 0u32;

    // First pass: collect all struct definitions for field ordering
    let mut struct_field_map = std::collections::HashMap::new();
    for child in root.children() {
        if child.kind() == SyntaxKind::StructDef
            && let Some((name, fields)) = lower_item::collect_struct_fields(&child, interner)
        {
            struct_field_map.insert(name, fields);
        }
    }

    // Second pass: lower all items (fn bodies can now reorder fields)
    for child in root.children() {
        match child.kind() {
            SyntaxKind::FnDef => {
                if let Some(item) = lower_item::lower_fn_def(
                    &child,
                    interner,
                    &mut local_def_counter,
                    &mut item_id_counter,
                    &mut bodies,
                    &mut body_owners,
                    diags,
                    &struct_field_map,
                ) {
                    items.push(item);
                }
            }
            SyntaxKind::StructDef => {
                if let Some(item) = lower_item::lower_struct_def(
                    &child,
                    interner,
                    &mut local_def_counter,
                    &mut item_id_counter,
                ) {
                    items.push(item);
                }
            }
            SyntaxKind::EnumDef => {
                if let Some(item) = lower_item::lower_enum_def(
                    &child,
                    interner,
                    &mut local_def_counter,
                    &mut item_id_counter,
                ) {
                    items.push(item);
                }
            }
            SyntaxKind::ExternBlock => {
                tracing::debug!("Processing ExternBlock");
                let mut stack = vec![child.clone()];
                while let Some(node) = stack.pop() {
                    tracing::debug!("  visiting node kind {:?}", node.kind());
                    if node.kind() == SyntaxKind::FnDef {
                        tracing::debug!("    found FnDef inside extern block");
                        if let Some(item) = lower_item::lower_fn_def(
                            &node,
                            interner,
                            &mut local_def_counter,
                            &mut item_id_counter,
                            &mut bodies,
                            &mut body_owners,
                            diags,
                            &struct_field_map,
                        ) {
                            items.push(item);
                        }
                    }
                    for inner_child in node.children() {
                        stack.push(inner_child);
                    }
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
