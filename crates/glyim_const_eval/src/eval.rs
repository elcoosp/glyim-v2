//! Constant expression evaluator.

use glyim_core::primitives::{BinOp, IntTy, UintTy, UnOp};
use glyim_hir::{Body, Expr, ExprId, Literal, MatchArm, Pat};
use glyim_span::Span;

use crate::{ConstEvalError, ConstEvalResult, ConstValue, MAX_EVAL_DEPTH};

/// The constant expression evaluator.
///
/// Evaluates HIR expressions at compile time to produce [`ConstValue`]s.
/// Supports:
/// - Integer, unsigned, float, bool, char, string, and unit literals
/// - Arithmetic binary operations (`+`, `-`, `*`, `/`, `%`)
/// - Comparison binary operations (`==`, `!=`, `<`, `>`, `<=`, `>=`)
/// - Logical binary operations (`&&`, `||`)
/// - Bitwise binary operations (`&`, `|`, `^`, `<<`, `>>`)
/// - Unary operations (`!`, `-`)
/// - `if` expressions
/// - `match` expressions
/// - Blocks (sequences of statements with optional tail expression)
pub struct ConstEvaluator<'a> {
    /// The HIR body containing expressions to evaluate.
    body: &'a Body,
}

impl<'a> ConstEvaluator<'a> {
    /// Create a new const evaluator for the given HIR body.
    ///
    /// The body contains all expressions and patterns that may be
    /// referenced during evaluation.
    pub fn new(body: &'a Body) -> Self {
        Self { body }
    }

    /// Evaluate an expression by its ID.
    ///
    /// This is the main entry point. It looks up the expression in the
    /// body and delegates evaluation.
    ///
    /// # Errors
    ///
    /// Returns a `ConstEvalError` if:
    /// - The expression is not evaluatable at compile time
    /// - Recursion depth exceeds the limit
    /// - An arithmetic overflow occurs
    /// - Division or remainder by zero is attempted
    pub fn evaluate(&self, expr_id: ExprId) -> ConstEvalResult<ConstValue> {
        let span = self.expr_span(expr_id);
        let expr = &self.body.exprs[expr_id];
        self.evaluate_expr(expr, span, 0)
    }

    /// Get the span for an expression ID.
    fn expr_span(&self, expr_id: ExprId) -> Span {
        self.body
            .expr_spans
            .get(expr_id)
            .copied()
            .unwrap_or(Span::DUMMY)
    }

    /// Evaluate an expression node with depth tracking.
    fn evaluate_expr(&self, expr: &Expr, span: Span, depth: u32) -> ConstEvalResult<ConstValue> {
        if depth >= MAX_EVAL_DEPTH {
            return Err(ConstEvalError::new(
                "const evaluation recursion limit exceeded",
                span,
            ));
        }

        match expr {
            Expr::Literal(lit) => self.eval_literal(lit, span),
            Expr::Binary { op, lhs, rhs } => self.eval_binary(*op, *lhs, *rhs, span, depth),
            Expr::Unary { op, expr } => self.eval_unary(*op, *expr, span, depth),
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => self.eval_if(*cond, *then_branch, *else_branch, span, depth),
            Expr::Match { scrutinee, arms } => self.eval_match(*scrutinee, arms, span, depth),
            Expr::Block { stmts, tail } => self.eval_block(stmts, *tail, depth),
            Expr::Tuple(elements) => {
                if elements.is_empty() {
                    Ok(ConstValue::Unit)
                } else {
                    Err(ConstEvalError::new(
                        "non-unit tuple expressions are not supported in const evaluation",
                        span,
                    ))
                }
            }
            Expr::Missing => Err(ConstEvalError::new(
                "missing expression in const evaluation",
                span,
            )),
            Expr::Err => Err(ConstEvalError::new(
                "error expression in const evaluation",
                span,
            )),
            _ => Err(ConstEvalError::new(
                "this expression kind is not supported in const evaluation",
                span,
            )),
        }
    }

    /// Evaluate a literal.
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

    /// Evaluate a binary operation.
    fn eval_binary(
        &self,
        op: BinOp,
        lhs_id: ExprId,
        rhs_id: ExprId,
        span: Span,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
        let lhs = self.evaluate_at_depth(lhs_id, depth)?;
        let rhs = self.evaluate_at_depth(rhs_id, depth)?;
        self.apply_binop(op, &lhs, &rhs, span)
    }

    /// Evaluate an expression, incrementing the recursion depth.
    fn evaluate_at_depth(&self, expr_id: ExprId, depth: u32) -> ConstEvalResult<ConstValue> {
        let span = self.expr_span(expr_id);
        let expr = &self.body.exprs[expr_id];
        self.evaluate_expr(expr, span, depth + 1)
    }

    /// Apply a binary operator to two evaluated values.
    fn apply_binop(
        &self,
        op: BinOp,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        match op {
            BinOp::Add => {
                let result = lhs.checked_add(rhs).ok_or_else(|| {
                    ConstEvalError::new("overflow in const addition or incompatible types", span)
                })?;
                result
                    .validate_range()
                    .ok_or_else(|| ConstEvalError::new("overflow in const addition", span))
            }
            BinOp::Sub => {
                let result = lhs.checked_sub(rhs).ok_or_else(|| {
                    ConstEvalError::new("overflow in const subtraction or incompatible types", span)
                })?;
                result
                    .validate_range()
                    .ok_or_else(|| ConstEvalError::new("overflow in const subtraction", span))
            }
            BinOp::Mul => {
                let result = lhs.checked_mul(rhs).ok_or_else(|| {
                    ConstEvalError::new(
                        "overflow in const multiplication or incompatible types",
                        span,
                    )
                })?;
                result
                    .validate_range()
                    .ok_or_else(|| ConstEvalError::new("overflow in const multiplication", span))
            }
            BinOp::Div => {
                self.check_div_by_zero(lhs, rhs, span, "division")?;
                let result = lhs.checked_div(rhs).ok_or_else(|| {
                    ConstEvalError::new("overflow in const division or incompatible types", span)
                })?;
                result
                    .validate_range()
                    .ok_or_else(|| ConstEvalError::new("overflow in const division", span))
            }
            BinOp::Rem => {
                self.check_div_by_zero(lhs, rhs, span, "remainder")?;
                let result = lhs.checked_rem(rhs).ok_or_else(|| {
                    ConstEvalError::new("overflow in const remainder or incompatible types", span)
                })?;
                result
                    .validate_range()
                    .ok_or_else(|| ConstEvalError::new("overflow in const remainder", span))
            }
            BinOp::Eq => self.compare_eq(lhs, rhs),
            BinOp::Ne => {
                let eq = self.compare_eq(lhs, rhs)?;
                eq.as_bool().map(|b| ConstValue::Bool(!b)).ok_or_else(|| {
                    ConstEvalError::new("internal error in const != comparison", span)
                })
            }
            BinOp::Lt => self.compare_lt(lhs, rhs, span),
            BinOp::Gt => self.compare_lt(rhs, lhs, span),
            BinOp::LtEq => {
                let gt = self.compare_lt(rhs, lhs, span)?;
                gt.as_bool().map(|b| ConstValue::Bool(!b)).ok_or_else(|| {
                    ConstEvalError::new("internal error in const <= comparison", span)
                })
            }
            BinOp::GtEq => {
                let lt = self.compare_lt(lhs, rhs, span)?;
                lt.as_bool().map(|b| ConstValue::Bool(!b)).ok_or_else(|| {
                    ConstEvalError::new("internal error in const >= comparison", span)
                })
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

    /// Check for division by zero.
    fn check_div_by_zero(
        &self,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
        op_name: &str,
    ) -> ConstEvalResult<()> {
        match (lhs, rhs) {
            (ConstValue::Int(_, _), ConstValue::Int(0, _))
            | (ConstValue::Uint(_, _), ConstValue::Uint(0, _)) => Err(ConstEvalError::new(
                format!("{} by zero in const evaluation", op_name),
                span,
            )),
            _ => Ok(()),
        }
    }

    /// Compare two values for equality, producing a `Bool`.
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

    /// Compare two values: lhs < rhs, producing a `Bool`.
    fn compare_lt(
        &self,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        let less = match (lhs, rhs) {
            (ConstValue::Int(a, _), ConstValue::Int(b, _)) => a < b,
            (ConstValue::Uint(a, _), ConstValue::Uint(b, _)) => a < b,
            (ConstValue::Char(a), ConstValue::Char(b)) => a < b,
            _ => {
                return Err(ConstEvalError::new(
                    "const < comparison requires matching numeric or char operands",
                    span,
                ));
            }
        };
        Ok(ConstValue::Bool(less))
    }

    /// Logical AND with short-circuit semantics.
    fn logical_and(
        &self,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        let a = lhs
            .as_bool()
            .ok_or_else(|| ConstEvalError::new("const && requires boolean operands", span))?;
        if !a {
            return Ok(ConstValue::Bool(false));
        }
        rhs.as_bool()
            .map(ConstValue::Bool)
            .ok_or_else(|| ConstEvalError::new("const && requires boolean operands", span))
    }

    /// Logical OR with short-circuit semantics.
    fn logical_or(
        &self,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        let a = lhs
            .as_bool()
            .ok_or_else(|| ConstEvalError::new("const || requires boolean operands", span))?;
        if a {
            return Ok(ConstValue::Bool(true));
        }
        rhs.as_bool()
            .map(ConstValue::Bool)
            .ok_or_else(|| ConstEvalError::new("const || requires boolean operands", span))
    }

    /// Bitwise AND.
    fn bitwise_and(
        &self,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => Ok(ConstValue::Int(a & b, *ty)),
            (ConstValue::Uint(a, ty), ConstValue::Uint(b, _)) => Ok(ConstValue::Uint(a & b, *ty)),
            (ConstValue::Bool(a), ConstValue::Bool(b)) => Ok(ConstValue::Bool(*a && *b)),
            _ => Err(ConstEvalError::new(
                "const & requires matching integer or boolean operands",
                span,
            )),
        }
    }

    /// Bitwise OR.
    fn bitwise_or(
        &self,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => Ok(ConstValue::Int(a | b, *ty)),
            (ConstValue::Uint(a, ty), ConstValue::Uint(b, _)) => Ok(ConstValue::Uint(a | b, *ty)),
            (ConstValue::Bool(a), ConstValue::Bool(b)) => Ok(ConstValue::Bool(*a || *b)),
            _ => Err(ConstEvalError::new(
                "const | requires matching integer or boolean operands",
                span,
            )),
        }
    }

    /// Bitwise XOR.
    fn bitwise_xor(
        &self,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => Ok(ConstValue::Int(a ^ b, *ty)),
            (ConstValue::Uint(a, ty), ConstValue::Uint(b, _)) => Ok(ConstValue::Uint(a ^ b, *ty)),
            (ConstValue::Bool(a), ConstValue::Bool(b)) => Ok(ConstValue::Bool(*a ^ *b)),
            _ => Err(ConstEvalError::new(
                "const ^ requires matching integer or boolean operands",
                span,
            )),
        }
    }

    /// Shift left.
    fn shift_left(
        &self,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => {
                let shift = (*b) as u32;
                Ok(ConstValue::Int(a.wrapping_shl(shift), *ty))
            }
            (ConstValue::Uint(a, ty), ConstValue::Int(b, _)) => {
                let shift = (*b) as u32;
                Ok(ConstValue::Uint(a.wrapping_shl(shift), *ty))
            }
            _ => Err(ConstEvalError::new(
                "const << requires integer operand and integer shift amount",
                span,
            )),
        }
    }

    /// Shift right.
    fn shift_right(
        &self,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => {
                let shift = (*b) as u32;
                Ok(ConstValue::Int(a.wrapping_shr(shift), *ty))
            }
            (ConstValue::Uint(a, ty), ConstValue::Int(b, _)) => {
                let shift = (*b) as u32;
                Ok(ConstValue::Uint(a.wrapping_shr(shift), *ty))
            }
            _ => Err(ConstEvalError::new(
                "const >> requires integer operand and integer shift amount",
                span,
            )),
        }
    }

    /// Evaluate a unary operation.
    fn eval_unary(
        &self,
        op: UnOp,
        expr_id: ExprId,
        span: Span,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
        let val = self.evaluate_at_depth(expr_id, depth)?;

        match op {
            UnOp::Not => val.not().ok_or_else(|| {
                ConstEvalError::new(
                    "const `!` operator requires a boolean or integer operand",
                    span,
                )
            }),
            UnOp::Neg => {
                let result = val.checked_neg().ok_or_else(|| {
                    ConstEvalError::new("const negation overflow or incompatible type", span)
                })?;
                result
                    .validate_range()
                    .ok_or_else(|| ConstEvalError::new("overflow in const negation", span))
            }
            UnOp::Deref => Err(ConstEvalError::new(
                "dereference is not supported in const evaluation",
                span,
            )),
        }
    }

    /// Evaluate an `if` expression.
    fn eval_if(
        &self,
        cond_id: ExprId,
        then_id: ExprId,
        else_id: Option<ExprId>,
        span: Span,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
        let cond_val = self.evaluate_at_depth(cond_id, depth)?;
        let cond_bool = cond_val
            .as_bool()
            .ok_or_else(|| ConstEvalError::new("const `if` condition must be a boolean", span))?;

        if cond_bool {
            self.evaluate_at_depth(then_id, depth)
        } else if let Some(else_id) = else_id {
            self.evaluate_at_depth(else_id, depth)
        } else {
            Ok(ConstValue::Unit)
        }
    }

    /// Evaluate a `match` expression.
    fn eval_match(
        &self,
        scrutinee_id: ExprId,
        arms: &[MatchArm],
        span: Span,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
        let scrutinee_val = self.evaluate_at_depth(scrutinee_id, depth)?;

        for arm in arms {
            if self.pattern_matches(&arm.pat, &scrutinee_val)? {
                return self.evaluate_at_depth(arm.body, depth);
            }
        }

        Err(ConstEvalError::new(
            "non-exhaustive match in const evaluation",
            span,
        ))
    }

    /// Check if a pattern matches a value.
    fn pattern_matches(
        &self,
        pat_id: &glyim_hir::PatId,
        value: &ConstValue,
    ) -> ConstEvalResult<bool> {
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
            _ => {
                tracing::debug!("pattern kind not supported in const match evaluation");
                Ok(false)
            }
        }
    }

    /// Evaluate a block expression.
    fn eval_block(
        &self,
        stmts: &[ExprId],
        tail: Option<ExprId>,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
        for stmt_id in stmts {
            self.evaluate_at_depth(*stmt_id, depth)?;
        }

        if let Some(tail_id) = tail {
            self.evaluate_at_depth(tail_id, depth)
        } else {
            Ok(ConstValue::Unit)
        }
    }
}
