use glyim_core::arena::IndexVec;
use glyim_core::interner::{Interner, Name};
use glyim_core::path::PathKind;
use glyim_core::primitives::*;
use glyim_diag::GlyimDiagnostic;
use glyim_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use std::collections::HashMap;

use crate::{
    Body, Expr, ExprId, Literal, MatchArm, Pat, PatId, Path as HirPath, PathSegment, Span,
};

use super::{
    first_ident_text, is_expr_node, is_type_node, lower_item::lower_param, lower_pat::lower_pat,
    lower_type::lower_type_ref, node_span,
};

pub(crate) fn lower_block_to_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
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
                    if (inner.kind() == SyntaxKind::FieldExpr
                        || inner.kind() == SyntaxKind::MethodCallExpr)
                        && let Some(base_id) = chain_base
                    {
                        if let Some(id) = lower_field_or_method_with_receiver(
                            &inner,
                            base_id,
                            interner,
                            body,
                            diags,
                            struct_field_map,
                        ) {
                            chain_base = Some(id);
                        }
                        continue;
                    }
                    let current = lower_expr(&inner, interner, body, diags, struct_field_map);
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
                let mut pat_node = None;
                let mut expr_node = None;
                for inner in child.children() {
                    if is_expr_node(&inner) || inner.kind() == SyntaxKind::Block {
                        expr_node = Some(inner.clone());
                    } else if inner.kind() == SyntaxKind::PatIdent
                        || inner.kind() == SyntaxKind::PatWild
                        || inner.kind() == SyntaxKind::PatTuple
                        || inner.kind() == SyntaxKind::PatStruct
                        || inner.kind() == SyntaxKind::PatOr
                    {
                        pat_node = Some(inner);
                    }
                }
                if let (Some(pat), Some(rhs)) = (pat_node, expr_node.clone())
                    && let Some(pat_id) = lower_pat(&pat, interner, &mut body.pats) {
                        let span = node_span(&child);
                        let lhs_expr_id = pat_to_expr(pat_id, body, interner, span);
                        let rhs_expr_id = lower_expr(&rhs, interner, body, diags, struct_field_map);
                        if let (Some(lhs_id), Some(rhs_id)) = (lhs_expr_id, rhs_expr_id) {
                            let assign = Expr::Assign {
                                lhs: lhs_id,
                                rhs: rhs_id,
                            };
                            let assign_id = body.alloc_expr(assign, node_span(&child));
                            if let Some(prev) = pending.take() {
                                stmts.push(prev);
                            }
                            stmts.push(assign_id);
                            pending = None;
                            last_has_semi = true;
                            continue;
                        }
                    }
                if let Some(rhs) = expr_node {
                    if let Some(prev) = pending.take() {
                        stmts.push(prev);
                    }
                    pending = lower_expr(&rhs, interner, body, diags, struct_field_map);
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
    
    body.alloc_expr(expr, node_span(node))
}

/// Convert a pattern into an expression (for LHS of assignment)
fn pat_to_expr(
    pat_id: PatId,
    body: &mut Body,
    _interner: &mut Interner,
    span: Span,
) -> Option<ExprId> {
    match &body.pats[pat_id] {
        Pat::Wild => None,
        Pat::Binding { name, .. } => {
            let path = HirPath::from_single(*name);
            let expr = Expr::Path(path);
            Some(body.alloc_expr(expr, span))
        }
        Pat::Path(path) => {
            let expr = Expr::Path(path.clone());
            Some(body.alloc_expr(expr, span))
        }
        Pat::Struct { path, .. } => {
            let expr = Expr::Path(path.clone());
            Some(body.alloc_expr(expr, span))
        }
        Pat::Tuple(_) | Pat::Or(_) | Pat::Literal(_) | Pat::Range { .. } | Pat::Err => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_field_or_method_with_receiver(
    node: &SyntaxNode,
    receiver_id: ExprId,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
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
                && let Some(id) = lower_expr(&child, interner, body, diags, struct_field_map)
            {
                arg_ids.push(id);
            }
        }
        let expr = Expr::MethodCall {
            receiver: receiver_id,
            method: name,
            args: arg_ids,
        };
        let eid = body.alloc_expr(expr, node_span(node));
        Some(eid)
    } else {
        let expr = Expr::Field {
            receiver: receiver_id,
            field: name,
        };
        let eid = body.alloc_expr(expr, node_span(node));
        Some(eid)
    }
}

pub(crate) fn lower_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    match node.kind() {
        SyntaxKind::Block => Some(lower_block_to_expr(
            node,
            interner,
            body,
            diags,
            struct_field_map,
        )),
        SyntaxKind::BinaryExpr => lower_binary_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::IfExpr => lower_if_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::PathExpr => lower_path_expr(node, interner, body),
        SyntaxKind::LitExpr => lower_lit_expr(node, interner, body),
        SyntaxKind::CallExpr => lower_call_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::MethodCallExpr => {
            lower_method_call_expr(node, interner, body, diags, struct_field_map)
        }
        SyntaxKind::UnaryExpr => lower_unary_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::RefExpr => lower_ref_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::MatchExpr => lower_match_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::WhileExpr => lower_while_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::LoopExpr => lower_loop_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::ForExpr => lower_for_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::AssignExpr => lower_assign_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::BreakExpr => lower_break_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::ContinueExpr => {
            let expr = Expr::Continue;
            let eid = body.alloc_expr(expr, node_span(node));
            Some(eid)
        }
        SyntaxKind::CastExpr => lower_cast_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::FieldExpr => lower_field_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::IndexExpr => lower_index_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::ArrayExpr => lower_array_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::TupleExpr => lower_tuple_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::RangeExpr => lower_range_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::ReturnExpr => lower_return_expr(node, interner, body, diags, struct_field_map),
        SyntaxKind::ClosureExpr => {
            lower_closure_expr(node, interner, body, diags, struct_field_map)
        }
        SyntaxKind::StructExpr => lower_struct_expr(node, interner, body, diags, struct_field_map),
        _ => {
            diags.push(GlyimDiagnostic::internal_error(format!(
                "Unhandled expression kind: {:?}",
                node.kind()
            )));
            None
        }
    }
}

fn lower_closure_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let mut params = Vec::new();
    let mut body_expr = None;
    for child in node.children() {
        match child.kind() {
            SyntaxKind::ParamList => {
                for param_node in child.children().filter(|c| c.kind() == SyntaxKind::Param) {
                    let (_, pat_id) = lower_param(&param_node, interner, &mut body.pats);
                    params.push(pat_id);
                }
            }
            _ if (is_expr_node(&child) || child.kind() == SyntaxKind::Block)
                && body_expr.is_none() =>
            {
                body_expr = lower_expr(&child, interner, body, diags, struct_field_map);
            }
            _ => {}
        }
    }
    let body_id = body_expr.unwrap_or_else(|| body.alloc_missing(node_span(node)));
    let expr = Expr::Closure {
        params,
        body: body_id,
    };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_struct_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let mut path = None;
    let mut fields = Vec::new();
    let mut spread = None;
    let mut in_braces = false;
    for child in node.children() {
        if child.kind() == SyntaxKind::PathExpr || child.kind() == SyntaxKind::UsePath {
            if path.is_none() {
                path = lower_path_expr(&child, interner, body);
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
                        if let Some(spread_id) =
                            lower_expr(&next, interner, body, diags, struct_field_map)
                        {
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
                let expr_node = child
                    .children()
                    .find(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block);
                if let Some(expr_id) =
                    expr_node.and_then(|n| lower_expr(&n, interner, body, diags, struct_field_map))
                {
                    fields.push((name, expr_id));
                }
            } else if is_expr_node(&child) && child.kind() != SyntaxKind::StructField {
                let name = interner.intern(child.text().to_string().trim());
                let expr_id = lower_expr(&child, interner, body, diags, struct_field_map);
                if let Some(eid) = expr_id {
                    fields.push((name, eid));
                }
            }
        }
    }
    let path_id = path.unwrap_or_else(|| body.alloc_missing(node_span(node)));
    let path_struct = if let Expr::Path(p) = &body.exprs[path_id] {
        p.clone()
    } else {
        HirPath {
            segments: vec![],
            kind: PathKind::Plain,
        }
    };
    let struct_name = path_struct.as_name();
    let ordered_fields = if let Some(name) = struct_name {
        if let Some(def_order) = struct_field_map.get(&name) {
            let mut ordered = Vec::new();
            for field_name in def_order {
                if let Some(pos) = fields.iter().position(|(f, _)| f == field_name) {
                    ordered.push(fields[pos]);
                }
            }
            for field in &fields {
                if !def_order.contains(&field.0) {
                    ordered.push(*field);
                }
            }
            ordered
        } else {
            fields
        }
    } else {
        fields
    };
    let expr = Expr::Struct {
        path: path_struct,
        fields: ordered_fields,
        spread,
    };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_binary_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
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
        let lhs_node = node
            .children_with_tokens()
            .take_while(|el| match el {
                glyim_syntax::SyntaxElement::Token(t) => t != &op_token,
                _ => true,
            })
            .filter_map(|el| el.as_node().cloned())
            .last()
            .filter(|n| is_expr_node(n) || n.kind() == SyntaxKind::Block);
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
            let lhs_id = lower_expr(&lhs, interner, body, diags, struct_field_map)?;
            let rhs_id = lower_expr(&rhs, interner, body, diags, struct_field_map)?;
            let op = lower_bin_op_token(&op_token);
            let expr = Expr::Binary {
                op,
                lhs: lhs_id,
                rhs: rhs_id,
            };
            let eid = body.alloc_expr(expr, node_span(node));
            return Some(eid);
        }
    }
    let expr_children: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)
        .collect();
    if expr_children.len() < 2 {
        return None;
    }
    let lhs_id = lower_expr(&expr_children[0], interner, body, diags, struct_field_map)?;
    let rhs_id = lower_expr(&expr_children[1], interner, body, diags, struct_field_map)?;
    let op = BinOp::Add;
    let expr = Expr::Binary {
        op,
        lhs: lhs_id,
        rhs: rhs_id,
    };
    let eid = body.alloc_expr(expr, node_span(node));
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
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
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
    let cond_id = lower_expr(&cond, interner, body, diags, struct_field_map)?;
    let then_id = lower_expr(&then_branch, interner, body, diags, struct_field_map)?;
    let else_id = else_branch.and_then(|e| lower_expr(&e, interner, body, diags, struct_field_map));
    let expr = Expr::If {
        cond: cond_id,
        then_branch: then_id,
        else_branch: else_id,
    };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_path_expr(node: &SyntaxNode, interner: &mut Interner, body: &mut Body) -> Option<ExprId> {
    let mut segments = Vec::new();
    for el in node.children_with_tokens() {
        if let glyim_syntax::SyntaxElement::Token(t) = el {
            if t.kind() == SyntaxKind::Ident {
                segments.push(PathSegment {
                    name: interner.intern(t.text()),
                    generic_args: None,
                });
            }
        } else if let glyim_syntax::SyntaxElement::Node(n) = el
            && n.kind() == SyntaxKind::UsePath
        {
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
    }
    if segments.is_empty() {
        return None;
    }
    let path = HirPath {
        segments,
        kind: PathKind::Plain,
    };
    let expr = Expr::Path(path);
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_lit_expr(node: &SyntaxNode, _interner: &mut Interner, body: &mut Body) -> Option<ExprId> {
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
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

pub(crate) fn lower_literal(token: &SyntaxToken) -> Literal {
    let text = token.text().to_string();
    match token.kind() {
        SyntaxKind::IntLit => {
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
        SyntaxKind::StringLit => Literal::Unit,
        _ => Literal::Unit,
    }
}

fn split_int_literal(s: &str) -> (String, Option<String>) {
    let mut i = 0;
    let chars: Vec<char> = s.chars().collect();
    if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
        i += 1;
    }
    if i + 1 < chars.len() && chars[i] == '0' {
        let prefix = chars[i + 1];
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
        (i128::from_str_radix(&s[2..], 16).unwrap_or(0), false)
    } else if s.starts_with("0o") || s.starts_with("0O") {
        (i128::from_str_radix(&s[2..], 8).unwrap_or(0), false)
    } else if s.starts_with("0b") || s.starts_with("0B") {
        (i128::from_str_radix(&s[2..], 2).unwrap_or(0), false)
    } else {
        (s.parse::<i128>().unwrap_or(0), s.starts_with('-'))
    }
}

fn split_float_literal(s: &str) -> (String, Option<String>) {
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
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let children: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_expr_node(c) || c.kind() == SyntaxKind::PathExpr)
        .collect();
    let func = children.first()?.clone();
    let args: Vec<SyntaxNode> = children.into_iter().skip(1).collect();
    let func_id = lower_expr(&func, interner, body, diags, struct_field_map)?;
    let mut arg_ids = Vec::new();
    for arg in args {
        if let Some(id) = lower_expr(&arg, interner, body, diags, struct_field_map) {
            arg_ids.push(id);
        }
    }
    let expr = Expr::Call {
        func: func_id,
        args: arg_ids,
    };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_method_call_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let receiver = node.children().find(|c| c.kind() == SyntaxKind::PathExpr)?;
    let receiver_id = lower_expr(&receiver, interner, body, diags, struct_field_map)?;
    let mut found_dot = false;
    let mut method_name = None;
    for el in node.children_with_tokens() {
        match el {
            glyim_syntax::SyntaxElement::Token(ref t) if t.kind() == SyntaxKind::Dot => {
                found_dot = true
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
    let mut arg_ids = Vec::new();
    for child in node.children() {
        if child.kind() != SyntaxKind::PathExpr
            && (is_expr_node(&child) || child.kind() == SyntaxKind::Block)
            && let Some(id) = lower_expr(&child, interner, body, diags, struct_field_map)
        {
            arg_ids.push(id);
        }
    }
    let expr = Expr::MethodCall {
        receiver: receiver_id,
        method,
        args: arg_ids,
    };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_unary_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
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
    let expr_id = lower_expr(&inner, interner, body, diags, struct_field_map)?;
    if op_token.kind() == SyntaxKind::And {
        let expr = Expr::Ref {
            expr: expr_id,
            mutability: Mutability::Not,
        };
        let eid = body.alloc_expr(expr, node_span(node));
        return Some(eid);
    }
    let op = match op_token.kind() {
        SyntaxKind::Bang => UnOp::Not,
        SyntaxKind::Minus => UnOp::Neg,
        SyntaxKind::Star => UnOp::Deref,
        _ => return None,
    };
    let expr = Expr::Unary { op, expr: expr_id };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_ref_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let inner = node
        .children()
        .find(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)?;
    let expr_id = lower_expr(&inner, interner, body, diags, struct_field_map)?;
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
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_match_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let scrutinee = node
        .children()
        .find(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)?;
    let scrutinee_id = lower_expr(&scrutinee, interner, body, diags, struct_field_map)?;
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
                        pat_id = lower_pat(&part, interner, &mut body.pats);
                    }
                    _ if is_expr_node(&part) => {
                        if body_id.is_none() {
                            body_id = lower_expr(&part, interner, body, diags, struct_field_map);
                        } else if guard.is_none() {
                            guard = lower_expr(&part, interner, body, diags, struct_field_map);
                        }
                    }
                    _ => {}
                }
            }
            if let (Some(pat), Some(body_id_val)) = (pat_id, body_id) {
                arms.push(MatchArm {
                    pat,
                    guard,
                    body: body_id_val,
                });
            }
        }
    }
    let expr = Expr::Match {
        scrutinee: scrutinee_id,
        arms,
    };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_while_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let mut children: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)
        .collect();
    if children.len() < 2 {
        return None;
    }
    let cond = children.remove(0);
    let body_expr = children.remove(0);
    let cond_id = lower_expr(&cond, interner, body, diags, struct_field_map)?;
    let body_id = lower_expr(&body_expr, interner, body, diags, struct_field_map)?;
    let expr = Expr::While {
        cond: cond_id,
        body: body_id,
    };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_loop_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let body_node = node.children().find(|c| c.kind() == SyntaxKind::Block)?;
    let body_id = lower_expr(&body_node, interner, body, diags, struct_field_map)?;
    let expr = Expr::Loop { body: body_id };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_for_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
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
    let pat_id = lower_pat(&pat_node, interner, &mut body.pats)?;
    let iterable_id = lower_expr(&iterable_node, interner, body, diags, struct_field_map)?;
    let body_id = lower_expr(&body_node, interner, body, diags, struct_field_map)?;
    let expr = Expr::For {
        pat: pat_id,
        iterable: iterable_id,
        body: body_id,
    };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_assign_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
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
    let lhs_id = lower_expr(&lhs, interner, body, diags, struct_field_map)?;
    let rhs_id = lower_expr(&rhs, interner, body, diags, struct_field_map)?;
    let expr = Expr::Assign {
        lhs: lhs_id,
        rhs: rhs_id,
    };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_return_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let value = node
        .children()
        .find(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)
        .and_then(|n| lower_expr(&n, interner, body, diags, struct_field_map));
    let expr = Expr::Return { value };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_break_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let value = node
        .children()
        .find(|c| is_expr_node(c) || c.kind() == SyntaxKind::Block)
        .and_then(|n| lower_expr(&n, interner, body, diags, struct_field_map));
    let expr = Expr::Break { value };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_cast_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
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
    let expr_id = lower_expr(&expr_node?, interner, body, diags, struct_field_map)?;
    let ty = lower_type_ref(&type_node?, interner)?;
    let expr = Expr::Cast { expr: expr_id, ty };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_field_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let receiver = node.children().find(|c| c.kind() == SyntaxKind::PathExpr)?;
    let receiver_id = lower_expr(&receiver, interner, body, diags, struct_field_map)?;
    let mut found_dot = false;
    let mut field_name = None;
    for el in node.children_with_tokens() {
        match el {
            glyim_syntax::SyntaxElement::Token(ref t) if t.kind() == SyntaxKind::Dot => {
                found_dot = true
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
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_index_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
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
    let base_id = lower_expr(&base, interner, body, diags, struct_field_map)?;
    let index_id = lower_expr(&index, interner, body, diags, struct_field_map)?;
    let expr = Expr::Index {
        base: base_id,
        index: index_id,
    };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_array_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let mut elems = Vec::new();
    for child in node.children().filter(is_expr_node) {
        if let Some(id) = lower_expr(&child, interner, body, diags, struct_field_map) {
            elems.push(id);
        }
    }
    let expr = Expr::Array(elems);
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_tuple_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let mut elems = Vec::new();
    for child in node.children().filter(is_expr_node) {
        if let Some(id) = lower_expr(&child, interner, body, diags, struct_field_map) {
            elems.push(id);
        }
    }
    let expr = Expr::Tuple(elems);
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}

fn lower_range_expr(
    node: &SyntaxNode,
    interner: &mut Interner,
    body: &mut Body,
    diags: &mut Vec<GlyimDiagnostic>,
    struct_field_map: &HashMap<Name, Vec<Name>>,
) -> Option<ExprId> {
    let children: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_expr_node(c) || c.kind() == SyntaxKind::LitExpr)
        .collect();
    let start = children
        .first()
        .and_then(|n| lower_expr(n, interner, body, diags, struct_field_map));
    let end = children
        .get(1)
        .and_then(|n| lower_expr(n, interner, body, diags, struct_field_map));
    let inclusive = node.children_with_tokens().any(
        |c| matches!(&c, glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::DotDotEq),
    );
    let expr = Expr::Range {
        start,
        end,
        inclusive,
    };
    let eid = body.alloc_expr(expr, node_span(node));
    Some(eid)
}
