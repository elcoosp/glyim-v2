pub(crate) mod lower_expr;
pub(crate) mod lower_item;
pub(crate) mod lower_pat;
pub(crate) mod lower_type;
pub(crate) mod lower_async;

#[cfg(test)]
pub(crate) use lower_expr::{lower_expr, lower_literal};

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

/// Like `first_ident_text` but descends into nested nodes (e.g. a closure
/// parameter `|n: i32|` is parsed as `Param -> PatIdent -> Ident`, so the
/// identifier is not a direct child of the `Param` node).
pub(crate) fn first_ident_text_with_depth(node: &SyntaxNode) -> Option<String> {
    for el in node.children_with_tokens() {
        match el {
            glyim_syntax::SyntaxElement::Token(t)
                if t.kind() == SyntaxKind::Ident =>
            {
                return Some(t.text().to_string());
            }
            glyim_syntax::SyntaxElement::Node(n) => {
                if let Some(found) = first_ident_text_with_depth(&n) {
                    return Some(found);
                }
            }
            _ => {}
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
            | SyntaxKind::AwaitExpr
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

/// Lower the parsed AST into a `CrateHir`, running the `async fn` / `.await`
/// desugar (`lower_async`) so the resulting HIR is the future state-machine
/// shape the type-checker understands. Used by the real compile pipeline
/// (`lower_crate_for_pipeline`) and by the lowering unit tests.
pub(crate) fn lower_crate(
    root: &SyntaxNode,
    interner: &mut Interner,
    diags: &mut Vec<GlyimDiagnostic>,
) -> CrateHir {
    let mut hir = lower_crate_raw(root, interner, diags);
    lower_async::desugar_async(&mut hir, diags);
    hir
}

/// Raw syntax → HIR lowering WITHOUT the `async fn` / `.await` desugar. Used by
/// `lower_crate_for_pipeline` (and thus the plan §6.1 plumbing tests, which
/// assert the `async fn` keyword lowers to `FnItem { is_async: true }` before
/// desugaring rewrites the item into a synchronous future-returning wrapper).
pub(crate) fn lower_crate_raw(
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
            SyntaxKind::ImplDef => {
                if let Some(item) = lower_item::lower_impl_def(
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
            SyntaxKind::TraitDef => {
                if let Some(item) = lower_item::lower_trait_def(
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
            SyntaxKind::Module => {
                if let Some(item) = lower_item::lower_mod_def(
                    &child,
                    interner,
                    &mut local_def_counter,
                    &mut item_id_counter,
                    &mut items,
                    &mut bodies,
                    &mut body_owners,
                    diags,
                    &struct_field_map,
                ) {
                    items.push(item);
                }
            }
            SyntaxKind::ConstDef => {
                if let Some(item) = lower_item::lower_const_def(
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
            // Other item kinds (Trait, Use, Extern, etc.) are not yet lowered.
            _ => {}
        }
    }

    let mut hir = CrateHir {
        items,
        bodies,
        body_owners,
        interner: interner.clone(),
    };

    hir
}
