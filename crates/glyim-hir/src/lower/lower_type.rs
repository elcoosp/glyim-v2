use glyim_core::interner::Interner;
use glyim_core::path::PathKind;
use glyim_core::primitives::*;
use glyim_syntax::{SyntaxKind, SyntaxNode};

use crate::{ConstRef, Path as HirPath, PathSegment, TypeRef};

use super::is_type_node;

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
                if is_type_node(&child)
                    && let Some(ty) = lower_type_ref(&child, interner)
                {
                    if after_arrow {
                        ret = Some(Box::new(ty));
                    } else {
                        params.push(ty);
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
            let mut inner = None;
            let mut len = None;
            for child in node.children() {
                if is_type_node(&child) {
                    inner = lower_type_ref(&child, interner);
                } else if child.kind() == SyntaxKind::LitExpr {
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
            let inner = node.children().find(is_type_node);
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

pub(crate) fn lower_path_from_type(node: &SyntaxNode, interner: &mut Interner) -> Option<HirPath> {
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
            kind: PathKind::Plain,
        })
    }
}
