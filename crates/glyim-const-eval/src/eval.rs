//! Constant expression evaluator.

use std::collections::HashMap;

use glyim_core::interner::Name;
use glyim_core::primitives::{BinOp, IntTy, UintTy, UnOp};
use glyim_hir::{Body, Expr, ExprId, Literal, MatchArm, Pat};
use glyim_span::Span;

use crate::{ConstEvalError, ConstEvalResult, ConstValue, MAX_EVAL_DEPTH};

/// The constant expression evaluator.
pub struct ConstEvaluator<'a> {
    body: &'a Body,
    env: Vec<HashMap<Name, ConstValue>>,
    pointer_width: u32,
}

impl<'a> ConstEvaluator<'a> {
    pub fn new(body: &'a Body) -> Self {
        Self {
            body,
            env: vec![HashMap::new()],
            pointer_width: 64, // Default to 64-bit
        }
    }

    pub fn with_pointer_width(mut self, width: u32) -> Self {
        self.pointer_width = width;
        self
    }

    pub fn evaluate(&mut self, expr_id: ExprId) -> ConstEvalResult<ConstValue> {
        let span = self.expr_span(expr_id);
        let expr = &self.body.exprs[expr_id];
        self.evaluate_expr(expr, span, 0)
    }

    fn expr_span(&self, expr_id: ExprId) -> Span {
        self.body.expr_spans.get(expr_id).copied().unwrap_or(Span::DUMMY)
    }

    fn lookup(&self, name: Name) -> Option<&ConstValue> {
        for scope in self.env.iter().rev() {
            if let Some(v) = scope.get(&name) {
                return Some(v);
            }
        }
        None
    }

    fn evaluate_expr(&mut self, expr: &Expr, span: Span, depth: u32) -> ConstEvalResult<ConstValue> {
        if depth >= MAX_EVAL_DEPTH {
            return Err(ConstEvalError::new("const evaluation recursion limit exceeded", span));
        }

        match expr {
            Expr::Literal(lit) => self.eval_literal(lit, span),
            Expr::Binary { op, lhs, rhs } => self.eval_binary(*op, *lhs, *rhs, span, depth),
            Expr::Unary { op, expr } => self.eval_unary(*op, *expr, span, depth),
            Expr::If { cond, then_branch, else_branch } => self.eval_if(*cond, *then_branch, *else_branch, span, depth),
            Expr::Match { scrutinee, arms } => self.eval_match(*scrutinee, arms, span, depth),
            Expr::Block { stmts, tail } => self.eval_block(stmts, *tail, depth),
            Expr::Tuple(elements) => {
                if elements.is_empty() {
                    return Ok(ConstValue::Unit);
                }
                let mut values = Vec::with_capacity(elements.len());
                for &elem_id in elements {
                    values.push(self.evaluate_at_depth(elem_id, depth)?);
                }
                Ok(ConstValue::Tuple(values))
            }
            Expr::Path(path) => {
                if let Some(name) = path.as_name() {
                    if let Some(val) = self.lookup(name) {
                        Ok(val.clone())
                    } else {
                        Err(ConstEvalError::new("path expressions not yet supported in const eval for non-local paths", span))
                    }
                } else {
                    Err(ConstEvalError::new("path expressions not yet supported in const eval for non-local paths", span))
                }
            }
            Expr::Field { receiver, field } => {
                let recv_val = self.evaluate_at_depth(*receiver, depth)?;
                if let ConstValue::Struct(fields) = recv_val {
                    fields.iter().find(|(n, _)| n == field)
                        .map(|(_, v)| v.clone())
                        .ok_or_else(|| ConstEvalError::new("field not found in struct", span))
                } else {
                    Err(ConstEvalError::new("field access on non-struct value", span))
                }
            }
            Expr::Index { base, index } => {
                let base_val = self.evaluate_at_depth(*base, depth)?;
                let idx_val = self.evaluate_at_depth(*index, depth)?;
                if let Some(idx) = idx_val.as_u128() {
                    match &base_val {
                        ConstValue::Array(arr) => {
                            if (idx as usize) < arr.len() {
                                Ok(arr[idx as usize].clone())
                            } else {
                                Err(ConstEvalError::new("index out of bounds in const eval", span))
                            }
                        }
                        ConstValue::Tuple(tup) => {
                            if (idx as usize) < tup.len() {
                                Ok(tup[idx as usize].clone())
                            } else {
                                Err(ConstEvalError::new("index out of bounds in const eval", span))
                            }
                        }
                        _ => Err(ConstEvalError::new("index access on non-array/tuple value", span)),
                    }
                } else {
                    Err(ConstEvalError::new("index is not an integer", span))
                }
            }
            Expr::Array(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for &elem_id in elements {
                    values.push(self.evaluate_at_depth(elem_id, depth)?);
                }
                Ok(ConstValue::Array(values))
            }
            Expr::Struct { fields, .. } => {
                let mut vals = Vec::with_capacity(fields.len());
                for (name, expr_id) in fields {
                    vals.push((*name, self.evaluate_at_depth(*expr_id, depth)?));
                }
                Ok(ConstValue::Struct(vals))
            }
            Expr::Cast { expr, ty } => {
                let val = self.evaluate_at_depth(*expr, depth)?;
                self.eval_cast(val, ty, span)
            }
            Expr::Ref { expr, .. } => {
                // In const eval, references are just the value itself (no memory model)
                self.evaluate_at_depth(*expr, depth)
            }
            Expr::Assign { lhs, rhs } => {
                let val = self.evaluate_at_depth(*rhs, depth)?;
                if let Expr::Path(p) = &self.body.exprs[*lhs] {
                    if let Some(name) = p.as_name() {
                        if let Some(scope) = self.env.last_mut() {
                            scope.insert(name, val.clone());
                            return Ok(ConstValue::Unit);
                        }
                    }
                }
                Err(ConstEvalError::new("assignment to non-path in const eval", span))
            }
            Expr::Call { .. } => Err(ConstEvalError::new("function calls not supported in const eval", span)),
            Expr::MethodCall { .. } => Err(ConstEvalError::new("method calls not supported in const eval", span)),
            Expr::Missing => Err(ConstEvalError::new("missing expression in const evaluation", span)),
            Expr::Err => Err(ConstEvalError::new("error expression in const evaluation", span)),
            Expr::Return { .. } => Err(ConstEvalError::new("return not supported in const eval", span)),
            Expr::Break { .. } => Err(ConstEvalError::new("break not supported in const eval", span)),
            Expr::Continue => Err(ConstEvalError::new("continue not supported in const eval", span)),
            Expr::Closure { .. } => Err(ConstEvalError::new("closures not supported in const eval", span)),
            Expr::Range { .. } => Err(ConstEvalError::new("ranges not supported in const eval", span)),
            Expr::While { .. } => Err(ConstEvalError::new("while loops not supported in const eval", span)),
            Expr::Loop { .. } => Err(ConstEvalError::new("loops not supported in const eval", span)),
            Expr::For { .. } => Err(ConstEvalError::new("for loops not supported in const eval", span)),
        }
    }

    fn eval_literal(&self, lit: &Literal, _span: Span) -> ConstEvalResult<ConstValue> {
        match lit {
            Literal::Int(val, Some(int_ty)) => Ok(ConstValue::Int(*val, *int_ty)),
            Literal::Int(val, None) => Ok(ConstValue::Int(*val, IntTy::I32)),
            Literal::Uint(val, Some(uint_ty)) => Ok(ConstValue::Uint(*val, *uint_ty)),
            Literal::Uint(val, None) => Ok(ConstValue::Uint(*val, UintTy::U32)),
            Literal::Float(bits, float_ty) => Ok(ConstValue::FloatBits(*bits, *float_ty)),
            Literal::Bool(b) => Ok(ConstValue::Bool(*b)),
            Literal::Char(c) => Ok(ConstValue::Char(*c)),
            Literal::String(name) => Ok(ConstValue::String(*name)),
            Literal::Unit => Ok(ConstValue::Unit),
        }
    }

    fn eval_binary(&mut self, op: BinOp, lhs_id: ExprId, rhs_id: ExprId, span: Span, depth: u32) -> ConstEvalResult<ConstValue> {
        let lhs = self.evaluate_at_depth(lhs_id, depth)?;
        let rhs = self.evaluate_at_depth(rhs_id, depth)?;
        self.apply_binop(op, &lhs, &rhs, span)
    }

    fn evaluate_at_depth(&mut self, expr_id: ExprId, depth: u32) -> ConstEvalResult<ConstValue> {
        let span = self.expr_span(expr_id);
        let expr = &self.body.exprs[expr_id];
        self.evaluate_expr(expr, span, depth + 1)
    }

    fn apply_binop(&self, op: BinOp, lhs: &ConstValue, rhs: &ConstValue, span: Span) -> ConstEvalResult<ConstValue> {
        match op {
            BinOp::Add => {
                let result = lhs.checked_add(rhs).ok_or_else(|| ConstEvalError::new("overflow in const addition or incompatible types", span))?;
                result.validate_range(self.pointer_width).ok_or_else(|| ConstEvalError::new("overflow in const addition", span))
            }
            BinOp::Sub => {
                let result = lhs.checked_sub(rhs).ok_or_else(|| ConstEvalError::new("overflow in const subtraction or incompatible types", span))?;
                result.validate_range(self.pointer_width).ok_or_else(|| ConstEvalError::new("overflow in const subtraction", span))
            }
            BinOp::Mul => {
                let result = lhs.checked_mul(rhs).ok_or_else(|| ConstEvalError::new("overflow in const multiplication or incompatible types", span))?;
                result.validate_range(self.pointer_width).ok_or_else(|| ConstEvalError::new("overflow in const multiplication", span))
            }
            BinOp::Div => {
                self.check_div_by_zero(lhs, rhs, span, "division")?;
                let result = lhs.checked_div(rhs).ok_or_else(|| ConstEvalError::new("overflow in const division or incompatible types", span))?;
                result.validate_range(self.pointer_width).ok_or_else(|| ConstEvalError::new("overflow in const division", span))
            }
            BinOp::Rem => {
                self.check_div_by_zero(lhs, rhs, span, "remainder")?;
                let result = lhs.checked_rem(rhs).ok_or_else(|| ConstEvalError::new("overflow in const remainder or incompatible types", span))?;
                result.validate_range(self.pointer_width).ok_or_else(|| ConstEvalError::new("overflow in const remainder", span))
            }
            BinOp::Eq => self.compare_eq(lhs, rhs),
            BinOp::Ne => {
                let eq = self.compare_eq(lhs, rhs)?;
                eq.as_bool().map(|b| ConstValue::Bool(!b)).ok_or_else(|| ConstEvalError::new("internal error in const != comparison", span))
            }
            BinOp::Lt => self.compare_lt(lhs, rhs, span),
            BinOp::Gt => self.compare_lt(rhs, lhs, span),
            BinOp::LtEq => {
                let gt = self.compare_lt(rhs, lhs, span)?;
                gt.as_bool().map(|b| ConstValue::Bool(!b)).ok_or_else(|| ConstEvalError::new("internal error in const <= comparison", span))
            }
            BinOp::GtEq => {
                let lt = self.compare_lt(lhs, rhs, span)?;
                lt.as_bool().map(|b| ConstValue::Bool(!b)).ok_or_else(|| ConstEvalError::new("internal error in const >= comparison", span))
            }
            BinOp::And => self.logical_and(lhs, rhs, span),
            BinOp::Or => self.logical_or(lhs, rhs, span),
            BinOp::BitAnd => self.bitwise_and(lhs, rhs, span),
            BinOp::BitOr => self.bitwise_or(lhs, rhs, span),
            BinOp::BitXor => self.bitwise_xor(lhs, rhs, span),
            BinOp::Shl => self.shift_left(lhs, rhs, span),
            BinOp::Shr => self.shift_right(lhs, rhs, span),
        }
    }

    fn check_div_by_zero(&self, lhs: &ConstValue, rhs: &ConstValue, span: Span, op_name: &str) -> ConstEvalResult<()> {
        match (lhs, rhs) {
            (ConstValue::Int(_, _), ConstValue::Int(0, _)) | (ConstValue::Uint(_, _), ConstValue::Uint(0, _)) => {
                Err(ConstEvalError::new(format!("{} by zero in const evaluation", op_name), span))
            }
            _ => Ok(()),
        }
    }

    fn compare_eq(&self, lhs: &ConstValue, rhs: &ConstValue) -> ConstEvalResult<ConstValue> {
        let equal = match (lhs, rhs) {
            (ConstValue::Int(a, _), ConstValue::Int(b, _)) => a == b,
            (ConstValue::Uint(a, _), ConstValue::Uint(b, _)) => a == b,
            (ConstValue::Bool(a), ConstValue::Bool(b)) => a == b,
            (ConstValue::Char(a), ConstValue::Char(b)) => a == b,
            (ConstValue::Unit, ConstValue::Unit) => true,
            _ => false,
        };
        Ok(ConstValue::Bool(equal))
    }

    fn compare_lt(&self, lhs: &ConstValue, rhs: &ConstValue, span: Span) -> ConstEvalResult<ConstValue> {
        let less = match (lhs, rhs) {
            (ConstValue::Int(a, _), ConstValue::Int(b, _)) => a < b,
            (ConstValue::Uint(a, _), ConstValue::Uint(b, _)) => a < b,
            (ConstValue::Char(a), ConstValue::Char(b)) => a < b,
            _ => return Err(ConstEvalError::new("const < comparison requires matching numeric or char operands", span)),
        };
        Ok(ConstValue::Bool(less))
    }

    fn logical_and(&self, lhs: &ConstValue, rhs: &ConstValue, span: Span) -> ConstEvalResult<ConstValue> {
        let a = lhs.as_bool().ok_or_else(|| ConstEvalError::new("const && requires boolean operands", span))?;
        if !a { return Ok(ConstValue::Bool(false)); }
        rhs.as_bool().map(ConstValue::Bool).ok_or_else(|| ConstEvalError::new("const && requires boolean operands", span))
    }

    fn logical_or(&self, lhs: &ConstValue, rhs: &ConstValue, span: Span) -> ConstEvalResult<ConstValue> {
        let a = lhs.as_bool().ok_or_else(|| ConstEvalError::new("const || requires boolean operands", span))?;
        if a { return Ok(ConstValue::Bool(true)); }
        rhs.as_bool().map(ConstValue::Bool).ok_or_else(|| ConstEvalError::new("const || requires boolean operands", span))
    }

    fn bitwise_and(&self, lhs: &ConstValue, rhs: &ConstValue, span: Span) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => Ok(ConstValue::Int(a & b, *ty)),
            (ConstValue::Uint(a, ty), ConstValue::Uint(b, _)) => Ok(ConstValue::Uint(a & b, *ty)),
            (ConstValue::Bool(a), ConstValue::Bool(b)) => Ok(ConstValue::Bool(*a && *b)),
            _ => Err(ConstEvalError::new("const & requires matching integer or boolean operands", span)),
        }
    }

    fn bitwise_or(&self, lhs: &ConstValue, rhs: &ConstValue, span: Span) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => Ok(ConstValue::Int(a | b, *ty)),
            (ConstValue::Uint(a, ty), ConstValue::Uint(b, _)) => Ok(ConstValue::Uint(a | b, *ty)),
            (ConstValue::Bool(a), ConstValue::Bool(b)) => Ok(ConstValue::Bool(*a || *b)),
            _ => Err(ConstEvalError::new("const | requires matching integer or boolean operands", span)),
        }
    }

    fn bitwise_xor(&self, lhs: &ConstValue, rhs: &ConstValue, span: Span) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => Ok(ConstValue::Int(a ^ b, *ty)),
            (ConstValue::Uint(a, ty), ConstValue::Uint(b, _)) => Ok(ConstValue::Uint(a ^ b, *ty)),
            (ConstValue::Bool(a), ConstValue::Bool(b)) => Ok(ConstValue::Bool(*a ^ *b)),
            _ => Err(ConstEvalError::new("const ^ requires matching integer or boolean operands", span)),
        }
    }

    fn shift_left(&self, lhs: &ConstValue, rhs: &ConstValue, span: Span) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => Ok(ConstValue::Int(a.wrapping_shl(*b as u32), *ty)),
            (ConstValue::Uint(a, ty), ConstValue::Int(b, _)) => Ok(ConstValue::Uint(a.wrapping_shl(*b as u32), *ty)),
            _ => Err(ConstEvalError::new("const << requires integer operand and integer shift amount", span)),
        }
    }

    fn shift_right(&self, lhs: &ConstValue, rhs: &ConstValue, span: Span) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => Ok(ConstValue::Int(a.wrapping_shr(*b as u32), *ty)),
            (ConstValue::Uint(a, ty), ConstValue::Int(b, _)) => Ok(ConstValue::Uint(a.wrapping_shr(*b as u32), *ty)),
            _ => Err(ConstEvalError::new("const >> requires integer operand and integer shift amount", span)),
        }
    }

    fn eval_unary(&mut self, op: UnOp, expr_id: ExprId, span: Span, depth: u32) -> ConstEvalResult<ConstValue> {
        let val = self.evaluate_at_depth(expr_id, depth)?;
        match op {
            UnOp::Not => val.not().ok_or_else(|| ConstEvalError::new("const `!` operator requires a boolean or integer operand", span)),
            UnOp::Neg => {
                let result = val.checked_neg().ok_or_else(|| ConstEvalError::new("const negation overflow or incompatible type", span))?;
                result.validate_range(self.pointer_width).ok_or_else(|| ConstEvalError::new("overflow in const negation", span))
            }
            UnOp::Deref => Err(ConstEvalError::new("dereference is not supported in const evaluation", span)),
        }
    }

    fn eval_if(&mut self, cond_id: ExprId, then_id: ExprId, else_id: Option<ExprId>, span: Span, depth: u32) -> ConstEvalResult<ConstValue> {
        let cond_val = self.evaluate_at_depth(cond_id, depth)?;
        let cond_bool = cond_val.as_bool().ok_or_else(|| ConstEvalError::new("const `if` condition must be a boolean", span))?;
        if cond_bool {
            self.evaluate_at_depth(then_id, depth)
        } else if let Some(else_id) = else_id {
            self.evaluate_at_depth(else_id, depth)
        } else {
            Ok(ConstValue::Unit)
        }
    }

    fn eval_match(&mut self, scrutinee_id: ExprId, arms: &[MatchArm], span: Span, depth: u32) -> ConstEvalResult<ConstValue> {
        let scrutinee_val = self.evaluate_at_depth(scrutinee_id, depth)?;
        for arm in arms {
            if self.pattern_matches(&arm.pat, &scrutinee_val)? {
                return self.evaluate_at_depth(arm.body, depth);
            }
        }
        Err(ConstEvalError::new("non-exhaustive match in const evaluation", span))
    }

    fn pattern_matches(&mut self, pat_id: &glyim_hir::PatId, value: &ConstValue) -> ConstEvalResult<bool> {
        let pat = &self.body.pats[*pat_id];
        match pat {
            Pat::Wild => Ok(true),
            Pat::Literal(lit) => {
                let pat_val = self.eval_literal(lit, Span::DUMMY)?;
                Ok(pat_val == *value)
            }
            Pat::Or(pats) => {
                for sub_pat in pats {
                    if self.pattern_matches(sub_pat, value)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Pat::Tuple(pats) => {
                if let ConstValue::Tuple(vals) = value {
                    if pats.len() != vals.len() { return Ok(false); }
                    for (p, v) in pats.iter().zip(vals.iter()) {
                        if !self.pattern_matches(p, v)? { return Ok(false); }
                    }
                    Ok(true)
                } else { Ok(false) }
            }
            Pat::Binding { name, .. } => {
                if let Some(scope) = self.env.last_mut() {
                    scope.insert(*name, value.clone());
                }
                Ok(true)
            }
            Pat::Struct { fields, .. } => {
                if let ConstValue::Struct(vals) = value {
                    if fields.len() != vals.len() { return Ok(false); }
                    for ((_, pat_id), (_, val)) in fields.iter().zip(vals.iter()) {
                        if !self.pattern_matches(pat_id, val)? { return Ok(false); }
                    }
                    Ok(true)
                } else { Ok(false) }
            }
            Pat::Path(path) => {
                if let Some(name) = path.as_name() {
                    if let Some(val) = self.lookup(name) {
                        Ok(val == value)
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            }
            Pat::Slice(pats) => {
                if let ConstValue::Array(vals) = value {
                    if pats.len() != vals.len() { return Ok(false); }
                    for (p, v) in pats.iter().zip(vals.iter()) {
                        if !self.pattern_matches(p, v)? { return Ok(false); }
                    }
                    Ok(true)
                } else { Ok(false) }
            }
            Pat::Range { start, end, inclusive } => {
                let start_val = if let Some(s) = start { self.eval_literal(s, Span::DUMMY)? } else { return Ok(false); };
                let end_val = if let Some(e) = end { self.eval_literal(e, Span::DUMMY)? } else { return Ok(false); };
                let ge_start = !self.compare_lt(value, &start_val, Span::DUMMY)?.as_bool().unwrap_or(false);
                let le_end = if *inclusive {
                    !self.compare_lt(&end_val, value, Span::DUMMY)?.as_bool().unwrap_or(false)
                } else {
                    self.compare_lt(value, &end_val, Span::DUMMY)?.as_bool().unwrap_or(false)
                };
                Ok(ge_start && le_end)
            }
            Pat::Err => Ok(false),
        }
    }

    fn eval_block(&mut self, stmts: &[ExprId], tail: Option<ExprId>, depth: u32) -> ConstEvalResult<ConstValue> {
        self.env.push(HashMap::new());
        for stmt_id in stmts {
            self.evaluate_at_depth(*stmt_id, depth)?;
        }
        let result = if let Some(tail_id) = tail {
            self.evaluate_at_depth(tail_id, depth)
        } else {
            Ok(ConstValue::Unit)
        };
        self.env.pop();
        result
    }

    fn eval_cast(&self, val: ConstValue, ty: &glyim_hir::TypeRef, span: Span) -> ConstEvalResult<ConstValue> {
        use glyim_core::primitives::{FloatTy, IntTy, UintTy};
        use glyim_hir::TypeRef;
        match ty {
            TypeRef::Path(path) => {
                if let Some(name) = path.as_name() {
                    let s = glyim_core::interner::Interner::default();
                    let n = s.resolve(name);
                    match n {
                        "i8" => val.as_i128().map(|v| ConstValue::Int(v as i8 as i128, IntTy::I8)).ok_or_else(|| ConstEvalError::new("cannot cast to i8", span)),
                        "i16" => val.as_i128().map(|v| ConstValue::Int(v as i16 as i128, IntTy::I16)).ok_or_else(|| ConstEvalError::new("cannot cast to i16", span)),
                        "i32" => val.as_i128().map(|v| ConstValue::Int(v as i32 as i128, IntTy::I32)).ok_or_else(|| ConstEvalError::new("cannot cast to i32", span)),
                        "i64" => val.as_i128().map(|v| ConstValue::Int(v as i64 as i128, IntTy::I64)).ok_or_else(|| ConstEvalError::new("cannot cast to i64", span)),
                        "u8" => val.as_u128().map(|v| ConstValue::Uint(v as u8 as u128, UintTy::U8)).ok_or_else(|| ConstEvalError::new("cannot cast to u8", span)),
                        "u16" => val.as_u128().map(|v| ConstValue::Uint(v as u16 as u128, UintTy::U16)).ok_or_else(|| ConstEvalError::new("cannot cast to u16", span)),
                        "u32" => val.as_u128().map(|v| ConstValue::Uint(v as u32 as u128, UintTy::U32)).ok_or_else(|| ConstEvalError::new("cannot cast to u32", span)),
                        "u64" => val.as_u128().map(|v| ConstValue::Uint(v as u64 as u128, UintTy::U64)).ok_or_else(|| ConstEvalError::new("cannot cast to u64", span)),
                        "f32" => val.as_f64().map(|v| ConstValue::FloatBits((v as f32).to_bits() as u64, FloatTy::F32)).ok_or_else(|| ConstEvalError::new("cannot cast to f32", span)),
                        "f64" => val.as_f64().map(|v| ConstValue::FloatBits(v.to_bits(), FloatTy::F64)).ok_or_else(|| ConstEvalError::new("cannot cast to f64", span)),
                        "bool" => Ok(ConstValue::Bool(val.as_i128().map(|v| v != 0).unwrap_or(false))),
                        _ => Err(ConstEvalError::new("unsupported cast target type", span)),
                    }
                } else {
                    Err(ConstEvalError::new("cannot cast to complex path", span))
                }
            }
            _ => Err(ConstEvalError::new("unsupported cast target type", span)),
        }
    }
}
