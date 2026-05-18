use glyim_core::arena::IndexVec;
use glyim_core::interner::Interner;
use glyim_core::path::PathKind;
use glyim_core::primitives::*;
use glyim_syntax::{SyntaxKind, SyntaxNode};

use crate::{Pat, PatId, Path as HirPath, PathSegment};

use super::{first_ident_text, lower_expr::lower_literal};

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
                    kind: PathKind::Plain,
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
                            let lit = lower_literal(&t); // t is &&SyntaxToken, need the deref
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
                        kind: PathKind::Plain,
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
                        kind: PathKind::Plain,
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
                        kind: PathKind::Plain,
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
                        kind: PathKind::Plain,
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
                kind: PathKind::Plain,
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
                kind: PathKind::Plain,
            };
            Some(pats.push(Pat::Path(path)))
        }
        _ => {
            tracing::warn!("STUB: unknown pattern kind {:?}", node.kind());
            Some(pats.push(Pat::Err))
        }
    }
}
