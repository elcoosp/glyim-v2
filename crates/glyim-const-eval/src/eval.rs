//! Constant expression evaluator.

use std::collections::HashMap;

use glyim_core::interner::{Interner, Name};
use glyim_core::primitives::{BinOp, IntTy, UintTy, UnOp};
use glyim_hir::{Body, Expr, ExprId, Literal, MatchArm, Pat};
use glyim_span::Span;

use crate::{ConstEvalError, ConstEvalResult, ConstValue, MAX_EVAL_DEPTH};

/// A user-defined `const fn` available to constant evaluation (plan §4.2).
///
/// The function's parameter patterns and body live inside the *same* `Body`
/// arena as the call site (they are allocated into the `Body` before the
/// evaluator is constructed), so `body` is a valid `ExprId`/`PatId` here. This
/// is the natural carrier for real `const fn`s once a body is available.
#[derive(Debug, Clone)]
pub struct BodyFn {
    /// Parameter patterns, in declaration order.
    pub params: Vec<glyim_hir::PatId>,
    /// Body expression id (in the same `Body` arena).
    pub body: glyim_hir::ExprId,
}

/// The constant expression evaluator.
pub struct ConstEvaluator<'a> {
    body: &'a Body,
    env: Vec<HashMap<Name, ConstValue>>,
    pointer_width: u32,
    /// Optional interner used to resolve path-named `const fn` callees to their
    /// source string for dispatch (plan §4.2). Without it, calls cannot be
    /// resolved.
    interner: Option<&'a Interner>,
    /// Registered user-defined `const fn`s (plan §4.2), keyed by name.
    const_fns: HashMap<Name, BodyFn>,
    /// Set when a `break`/`continue` is encountered inside a loop so the loop
    /// driver can react. `None` means normal flow.
    loop_control: Option<LoopControl>,
}

/// Control-flow signal raised by `break`/`continue` during constant
/// evaluation. The loop driver consumes it after evaluating a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoopControl {
    Break,
    Continue,
}

/// A `const fn` available to constant evaluation.
///
/// The first concrete carrier for plan §4.2 (const-evaluation of function
/// calls). Calls to a path whose name maps to one of these are evaluated
/// directly; this is the hook real `const fn`s (whether user-defined or from
/// the std library) will plug into later — the `ConstValue` arms below list
/// every value kind so new builtins are easy to add.
#[derive(Debug, Clone)]
pub enum ConstFn {
    /// `abs(x)` — absolute value of a signed integer.
    Abs,
    /// `min(a, b)` — smaller of two integers of the same type.
    Min,
    /// `max(a, b)` — larger of two integers of the same type.
    Max,
    /// `sqrt(x)` — integer square root of an unsigned integer.
    Sqrt,
    /// `is_power_of_two(x)` — whether an unsigned integer is a power of two.
    IsPowerOfTwo,
}

impl ConstFn {
    /// Resolve a crate-visible builtin name to its `ConstFn`.
    pub(crate) fn from_name(name: &str) -> Option<ConstFn> {
        match name {
            "abs" => Some(ConstFn::Abs),
            "min" => Some(ConstFn::Min),
            "max" => Some(ConstFn::Max),
            "sqrt" => Some(ConstFn::Sqrt),
            "is_power_of_two" => Some(ConstFn::IsPowerOfTwo),
            _ => None,
        }
    }

    /// Evaluate this builtin against already-evaluated arguments.
    pub(crate) fn apply(&self, args: &[ConstValue], span: Span) -> ConstEvalResult<ConstValue> {
        match self {
            ConstFn::Abs => {
                let x = args.first().ok_or_else(|| {
                    ConstEvalError::new("abs: expected 1 argument", span)
                })?;
                match x {
                    ConstValue::Int(v, ty) => Ok(ConstValue::Int(v.abs(), *ty)),
                    _ => Err(ConstEvalError::new(
                        "abs: argument must be a signed integer",
                        span,
                    )),
                }
            }
            ConstFn::Min => {
                let (a, b) = two_args(args, span)?;
                match (a, b) {
                    (ConstValue::Int(x, tx), ConstValue::Int(y, ty)) if tx == ty => {
                        Ok(ConstValue::Int(*x.min(y), *tx))
                    }
                    (ConstValue::Uint(x, tx), ConstValue::Uint(y, ty)) if tx == ty => {
                        Ok(ConstValue::Uint(*x.min(y), *tx))
                    }
                    _ => Err(ConstEvalError::new(
                        "min: arguments must be integers of the same type",
                        span,
                    )),
                }
            }
            ConstFn::Max => {
                let (a, b) = two_args(args, span)?;
                match (a, b) {
                    (ConstValue::Int(x, tx), ConstValue::Int(y, ty)) if tx == ty => {
                        Ok(ConstValue::Int(*x.max(y), *tx))
                    }
                    (ConstValue::Uint(x, tx), ConstValue::Uint(y, ty)) if tx == ty => {
                        Ok(ConstValue::Uint(*x.max(y), *tx))
                    }
                    _ => Err(ConstEvalError::new(
                        "max: arguments must be integers of the same type",
                        span,
                    )),
                }
            }
            ConstFn::Sqrt => {
                let x = args.first().ok_or_else(|| {
                    ConstEvalError::new("sqrt: expected 1 argument", span)
                })?;
                match x {
                    ConstValue::Uint(v, ty) => Ok(ConstValue::Uint((*v as f64).sqrt() as u128, *ty)),
                    ConstValue::Int(v, ty) if *v >= 0 => {
                        Ok(ConstValue::Int(((*v as f64).sqrt() as i128).abs(), *ty))
                    }
                    _ => Err(ConstEvalError::new(
                        "sqrt: argument must be a non-negative integer",
                        span,
                    )),
                }
            }
            ConstFn::IsPowerOfTwo => {
                let x = args.first().ok_or_else(|| {
                    ConstEvalError::new("is_power_of_two: expected 1 argument", span)
                })?;
                match x {
                    ConstValue::Uint(v, _ty) => Ok(ConstValue::Bool(v.is_power_of_two())),
                    ConstValue::Int(v, ty) if *v >= 0 => {
                        Ok(ConstValue::Bool((*v as u128).is_power_of_two()))
                    }
                    _ => Err(ConstEvalError::new(
                        "is_power_of_two: argument must be a non-negative integer",
                        span,
                    )),
                }
            }
        }
    }
}

/// Helper: pull exactly two arguments out of an evaluated-args slice.
fn two_args(
    args: &[ConstValue],
    span: Span,
) -> ConstEvalResult<(&ConstValue, &ConstValue)> {
    if args.len() != 2 {
        return Err(ConstEvalError::new(
            "builtin expects exactly 2 arguments",
            span,
        ));
    }
    Ok((&args[0], &args[1]))
}

impl<'a> ConstEvaluator<'a> {
    pub fn new(body: &'a Body) -> Self {
        Self {
            body,
            env: vec![HashMap::new()],
            pointer_width: 64, // Default to 64-bit
            interner: None,
            const_fns: HashMap::new(),
            loop_control: None,
        }
    }

    /// Attach the interner used to resolve path-named `const fn` callees
    /// (plan §4.2). Required for `Expr::Call`/`Expr::MethodCall` evaluation.
    pub fn with_interner(mut self, interner: &'a Interner) -> Self {
        self.interner = Some(interner);
        self
    }

    /// Register a user-defined `const fn` (plan §4.2). Its parameter patterns
    /// and body must already live in the `Body` arena passed to `new`.
    pub fn with_const_fn(mut self, name: Name, f: BodyFn) -> Self {
        self.const_fns.insert(name, f);
        self
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
        self.body
            .expr_spans
            .get(expr_id)
            .copied()
            .unwrap_or(Span::DUMMY)
    }

    fn lookup(&self, name: Name) -> Option<&ConstValue> {
        for scope in self.env.iter().rev() {
            if let Some(v) = scope.get(&name) {
                return Some(v);
            }
        }
        None
    }

    /// Assign `val` to `name` in the scope stack.
    ///
    /// If `name` already exists in some enclosing scope, the assignment
    /// *updates that scope* — so a mutable variable defined outside a block
    /// (e.g. the accumulator in `for x in iter { acc = acc + x }`, where the
    /// loop body is a block) keeps its value across the block boundary. Only a
    /// genuinely new name (not yet present anywhere) is inserted into the
    /// innermost scope. This mirrors how `lookup` already resolves reads from
    /// the back of the scope stack.
    fn assign_name(&mut self, name: Name, val: ConstValue) {
        // Update the deepest scope that already defines `name` (so mutations of
        // an outer variable persist across block boundaries); if `name` is new
        // everywhere, define it in the innermost scope.
        if let Some(idx) = self
            .env
            .iter()
            .rposition(|scope| scope.contains_key(&name))
        {
            self.env[idx].insert(name, val);
            return;
        }
        self.env.last_mut().expect("const-eval env is never empty").insert(name, val);
    }

    fn evaluate_expr(
        &mut self,
        expr: &Expr,
        span: Span,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
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
                        Err(ConstEvalError::new(
                            "path expressions not yet supported in const eval for non-local paths",
                            span,
                        ))
                    }
                } else {
                    Err(ConstEvalError::new(
                        "path expressions not yet supported in const eval for non-local paths",
                        span,
                    ))
                }
            }
            Expr::Field { receiver, field } => {
                let recv_val = self.evaluate_at_depth(*receiver, depth)?;
                if let ConstValue::Struct(fields) = recv_val {
                    fields
                        .iter()
                        .find(|(n, _)| n == field)
                        .map(|(_, v)| v.clone())
                        .ok_or_else(|| ConstEvalError::new("field not found in struct", span))
                } else {
                    Err(ConstEvalError::new(
                        "field access on non-struct value",
                        span,
                    ))
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
                                Err(ConstEvalError::new(
                                    "index out of bounds in const eval",
                                    span,
                                ))
                            }
                        }
                        ConstValue::Tuple(tup) => {
                            if (idx as usize) < tup.len() {
                                Ok(tup[idx as usize].clone())
                            } else {
                                Err(ConstEvalError::new(
                                    "index out of bounds in const eval",
                                    span,
                                ))
                            }
                        }
                        _ => Err(ConstEvalError::new(
                            "index access on non-array/tuple value",
                            span,
                        )),
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
                if let Expr::Path(p) = &self.body.exprs[*lhs]
                    && let Some(name) = p.as_name()
                {
                    self.assign_name(name, val);
                    return Ok(ConstValue::Unit);
                }
                Err(ConstEvalError::new(
                    "assignment to non-path in const eval",
                    span,
                ))
            }
            Expr::Call { func, args } => {
                // Plan §4.2: const-evaluation of function calls. Two carriers:
                //   * an immediately-invoked closure (`(|p| body)(a)`), and
                //   * a path-named callee that is either a registered
                //     user-defined `const fn` or a builtin `ConstFn`.
                let callee = &self.body.exprs[*func];
                if let Expr::Closure {
                    params, body, ..
                } = callee
                {
                    return self.eval_closure_call(params, *body, args, span, depth);
                }
                let name = match callee {
                    Expr::Path(p) => p.as_name(),
                    _ => None,
                };
                let name = match name {
                    Some(n) => n,
                    None => {
                        return Err(ConstEvalError::new(
                            "only path-named const fns and closures are supported in const eval",
                            span,
                        ))
                    }
                };
                // Evaluate arguments once; shared by user-fn and builtin paths.
                let mut arg_vals = Vec::with_capacity(args.len());
                for &a in args {
                    arg_vals.push(self.evaluate_at_depth(a, depth)?);
                }
                // User-defined const fn (registered body in the same arena)?
                if let Some(f) = self.const_fns.get(&name).cloned() {
                    return self.eval_user_const_fn(&f, &arg_vals, span, depth);
                }
                // Builtin const fn dispatched by path name.
                let interner = self.interner.ok_or_else(|| {
                    ConstEvalError::new(
                        "const evaluator has no interner; cannot resolve const fn name",
                        span,
                    )
                })?;
                let name_str = interner.resolve(name);
                let cf = ConstFn::from_name(name_str).ok_or_else(|| {
                    ConstEvalError::new(
                        format!("unknown const fn `{}`", name_str),
                        span,
                    )
                })?;
                cf.apply(&arg_vals, span)
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => {
                // Plan §4.2: const-evaluation of method calls. The builtin
                // `const fn`s are also available as methods on their receiver
                // (e.g. `x.abs()`, `a.min(b)`). The receiver is prepended to the
                // argument list and dispatched via the same `ConstFn` table.
                let recv_val = self.evaluate_at_depth(*receiver, depth)?;
                let interner = self.interner.ok_or_else(|| {
                    ConstEvalError::new(
                        "const evaluator has no interner; cannot resolve method name",
                        span,
                    )
                })?;
                let method_str = interner.resolve(*method);
                let cf = ConstFn::from_name(method_str).ok_or_else(|| {
                    ConstEvalError::new(
                        format!("unknown const method `{}`", method_str),
                        span,
                    )
                })?;
                let mut arg_vals = Vec::with_capacity(args.len() + 1);
                arg_vals.push(recv_val);
                for &a in args {
                    arg_vals.push(self.evaluate_at_depth(a, depth)?);
                }
                cf.apply(&arg_vals, span)
            }
            Expr::Missing => Err(ConstEvalError::new(
                "missing expression in const evaluation",
                span,
            )),
            Expr::Err => Err(ConstEvalError::new(
                "error expression in const evaluation",
                span,
            )),
            Expr::Return { value } => match value {
                Some(v) => self.evaluate_at_depth(*v, depth),
                None => Ok(ConstValue::Unit),
            },
            Expr::Break { value } => {
                // Signal the enclosing loop driver; the value (if any) is
                // currently ignored by the loop drivers, matching `break`
                // without a label in constant evaluation.
                let _ = value;
                self.loop_control = Some(LoopControl::Break);
                Ok(ConstValue::Unit)
            }
            Expr::Continue => {
                self.loop_control = Some(LoopControl::Continue);
                Ok(ConstValue::Unit)
            }
            Expr::Closure { .. } => Err(ConstEvalError::new(
                "a bare closure is not a const value; invoke it immediately, e.g. (|x| x + 1)(2)",
                span,
            )),
            Expr::Range { start, end, inclusive } => {
                let start_val = match start {
                    Some(id) => Some(Box::new(self.evaluate_at_depth(*id, depth)?)),
                    None => None,
                };
                let end_val = match end {
                    Some(id) => Some(Box::new(self.evaluate_at_depth(*id, depth)?)),
                    None => None,
                };
                Ok(ConstValue::Range(start_val, end_val, *inclusive))
            }
            Expr::While { cond, body } => self.eval_while(*cond, *body, span, depth),
            Expr::Loop { body } => self.eval_loop(*body, span, depth),
            Expr::For {
                pat,
                iterable,
                body,
            } => self.eval_for(*pat, *iterable, *body, span, depth),
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

    fn eval_binary(
        &mut self,
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

    fn evaluate_at_depth(&mut self, expr_id: ExprId, depth: u32) -> ConstEvalResult<ConstValue> {
        let span = self.expr_span(expr_id);
        let expr = &self.body.exprs[expr_id];
        self.evaluate_expr(expr, span, depth + 1)
    }

    /// Evaluate an immediately-invoked closure `(|params| body)(args)`: bind
    /// the evaluated arguments to the closure's parameter patterns in a fresh
    /// scope, then evaluate the body (plan §4.3).
    fn eval_closure_call(
        &mut self,
        params: &[glyim_hir::PatId],
        body: ExprId,
        args: &[ExprId],
        span: Span,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
        if params.len() != args.len() {
            return Err(ConstEvalError::new(
                format!(
                    "closure invoked with {} argument(s) but expects {}",
                    args.len(),
                    params.len()
                ),
                span,
            ));
        }
        let mut arg_vals = Vec::with_capacity(args.len());
        for &a in args {
            arg_vals.push(self.evaluate_at_depth(a, depth)?);
        }
        self.bind_and_eval(params, body, &arg_vals, span, depth)
    }

    /// Evaluate a user-defined `const fn` (plan §4.2): bind the evaluated
    /// arguments to the registered parameter patterns in a fresh scope and
    /// evaluate the body.
    fn eval_user_const_fn(
        &mut self,
        f: &BodyFn,
        arg_vals: &[ConstValue],
        span: Span,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
        if f.params.len() != arg_vals.len() {
            return Err(ConstEvalError::new(
                format!(
                    "const fn invoked with {} argument(s) but expects {}",
                    arg_vals.len(),
                    f.params.len()
                ),
                span,
            ));
        }
        self.bind_and_eval(&f.params, f.body, arg_vals, span, depth)
    }

    /// Shared tail of closure/fn invocation: push a scope, bind `arg_vals` to
    /// `params` (via pattern matching), evaluate `body`, pop the scope, and
    /// return the body's value.
    fn bind_and_eval(
        &mut self,
        params: &[glyim_hir::PatId],
        body: ExprId,
        arg_vals: &[ConstValue],
        _span: Span,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
        self.env.push(HashMap::new());
        for (pat_id, val) in params.iter().zip(arg_vals.iter()) {
            self.pattern_matches(pat_id, val)?;
        }
        let result = self.evaluate_at_depth(body, depth);
        self.env.pop();
        result
    }

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
                    .validate_range(self.pointer_width)
                    .ok_or_else(|| ConstEvalError::new("overflow in const addition", span))
            }
            BinOp::Sub => {
                let result = lhs.checked_sub(rhs).ok_or_else(|| {
                    ConstEvalError::new("overflow in const subtraction or incompatible types", span)
                })?;
                result
                    .validate_range(self.pointer_width)
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
                    .validate_range(self.pointer_width)
                    .ok_or_else(|| ConstEvalError::new("overflow in const multiplication", span))
            }
            BinOp::Div => {
                self.check_div_by_zero(lhs, rhs, span, "division")?;
                let result = lhs.checked_div(rhs).ok_or_else(|| {
                    ConstEvalError::new("overflow in const division or incompatible types", span)
                })?;
                result
                    .validate_range(self.pointer_width)
                    .ok_or_else(|| ConstEvalError::new("overflow in const division", span))
            }
            BinOp::Rem => {
                self.check_div_by_zero(lhs, rhs, span, "remainder")?;
                let result = lhs.checked_rem(rhs).ok_or_else(|| {
                    ConstEvalError::new("overflow in const remainder or incompatible types", span)
                })?;
                result
                    .validate_range(self.pointer_width)
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

    fn shift_left(
        &self,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => {
                Ok(ConstValue::Int(a.wrapping_shl(*b as u32), *ty))
            }
            (ConstValue::Uint(a, ty), ConstValue::Int(b, _)) => {
                Ok(ConstValue::Uint(a.wrapping_shl(*b as u32), *ty))
            }
            _ => Err(ConstEvalError::new(
                "const << requires integer operand and integer shift amount",
                span,
            )),
        }
    }

    fn shift_right(
        &self,
        lhs: &ConstValue,
        rhs: &ConstValue,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        match (lhs, rhs) {
            (ConstValue::Int(a, ty), ConstValue::Int(b, _)) => {
                Ok(ConstValue::Int(a.wrapping_shr(*b as u32), *ty))
            }
            (ConstValue::Uint(a, ty), ConstValue::Int(b, _)) => {
                Ok(ConstValue::Uint(a.wrapping_shr(*b as u32), *ty))
            }
            _ => Err(ConstEvalError::new(
                "const >> requires integer operand and integer shift amount",
                span,
            )),
        }
    }

    fn eval_unary(
        &mut self,
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
                    .validate_range(self.pointer_width)
                    .ok_or_else(|| ConstEvalError::new("overflow in const negation", span))
            }
            UnOp::Deref => Err(ConstEvalError::new(
                "dereference is not supported in const evaluation",
                span,
            )),
        }
    }

    fn eval_if(
        &mut self,
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

    fn eval_match(
        &mut self,
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

    fn pattern_matches(
        &mut self,
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
            Pat::Tuple(pats) => {
                if let ConstValue::Tuple(vals) = value {
                    if pats.len() != vals.len() {
                        return Ok(false);
                    }
                    for (p, v) in pats.iter().zip(vals.iter()) {
                        if !self.pattern_matches(p, v)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Pat::Binding { name, .. } => {
                if let Some(scope) = self.env.last_mut() {
                    scope.insert(*name, value.clone());
                }
                Ok(true)
            }
            Pat::Struct { fields, .. } => {
                if let ConstValue::Struct(vals) = value {
                    if fields.len() != vals.len() {
                        return Ok(false);
                    }
                    for ((_, pat_id), (_, val)) in fields.iter().zip(vals.iter()) {
                        if !self.pattern_matches(pat_id, val)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                } else {
                    Ok(false)
                }
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
                    if pats.len() != vals.len() {
                        return Ok(false);
                    }
                    for (p, v) in pats.iter().zip(vals.iter()) {
                        if !self.pattern_matches(p, v)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Pat::Range {
                start,
                end,
                inclusive,
            } => {
                let start_val = if let Some(s) = start {
                    self.eval_literal(s, Span::DUMMY)?
                } else {
                    return Ok(false);
                };
                let end_val = if let Some(e) = end {
                    self.eval_literal(e, Span::DUMMY)?
                } else {
                    return Ok(false);
                };
                let ge_start = !self
                    .compare_lt(value, &start_val, Span::DUMMY)?
                    .as_bool()
                    .unwrap_or(false);
                let le_end = if *inclusive {
                    !self
                        .compare_lt(&end_val, value, Span::DUMMY)?
                        .as_bool()
                        .unwrap_or(false)
                } else {
                    self.compare_lt(value, &end_val, Span::DUMMY)?
                        .as_bool()
                        .unwrap_or(false)
                };
                Ok(ge_start && le_end)
            }
            Pat::Err => Ok(false),
        }
    }

    fn eval_block(
        &mut self,
        stmts: &[ExprId],
        tail: Option<ExprId>,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
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

    /// Constant-fold a `while` loop: evaluate the condition; while it holds,
    /// evaluate the body (which may `break`/`continue`). A `break` exits the
    /// loop and yields `Unit`; a `continue` skips to the next iteration. This
    /// only terminates for constant loop conditions (the common case for
    /// compile-time computation).
    fn eval_while(
        &mut self,
        cond_id: ExprId,
        body_id: ExprId,
        span: Span,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
        // Bound iterations so a non-terminating loop cannot hang evaluation.
        const MAX_ITERS: u32 = 1_000_000;
        for _ in 0..MAX_ITERS {
            let cond_val = self.evaluate_at_depth(cond_id, depth)?;
            let cond_bool = cond_val.as_bool().ok_or_else(|| {
                ConstEvalError::new("const `while` condition must be a boolean", span)
            })?;
            if !cond_bool {
                self.loop_control = None;
                return Ok(ConstValue::Unit);
            }
            self.evaluate_at_depth(body_id, depth)?;
            match self.loop_control.take() {
                Some(LoopControl::Break) => return Ok(ConstValue::Unit),
                Some(LoopControl::Continue) => continue,
                None => continue,
            }
        }
        Err(ConstEvalError::new(
            "const `while` loop exceeded iteration limit (non-constant condition?)",
            span,
        ))
    }

    /// Constant-fold an (infinite) `loop` body until a `break` is hit. A
    /// `continue` simply re-runs the body. A loop with no `break` is bounded by
    /// an iteration cap so evaluation cannot hang.
    fn eval_loop(
        &mut self,
        body_id: ExprId,
        span: Span,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
        const MAX_ITERS: u32 = 1_000_000;
        for _ in 0..MAX_ITERS {
            self.evaluate_at_depth(body_id, depth)?;
            match self.loop_control.take() {
                Some(LoopControl::Break) => return Ok(ConstValue::Unit),
                Some(LoopControl::Continue) => continue,
                None => continue,
            }
        }
        Err(ConstEvalError::new(
            "const `loop` exceeded iteration limit (no terminating `break`?)",
            span,
        ))
    }

    /// Constant-fold a `for` loop over an *already const-evaluable* iterable.
    ///
    /// The desugaring the language uses at runtime (`IntoIterator::into_iter`,
    /// then a loop calling `.next()`) requires `Expr::Call`/`Expr::MethodCall`,
    /// which the const evaluator does not yet implement. For the common
    /// compile-time cases — `for x in RANGE { .. }`, `for x in ARRAY { .. }`,
    /// `for (a, b) in TUPLE { .. }` — the iterable is itself a `ConstValue`, so
    /// we can drive the loop directly off that value without resorting to the
    /// (unimplemented) call machinery. This is the slice the plan's "CRC table
    /// via a `for` loop and `Range`" example depends on (§4.4).
    ///
    /// `break` exits the loop and yields `Unit`; `continue` advances to the next
    /// element. Iteration is bounded by `MAX_ITERS` so a non-terminating loop
    /// (e.g. `for _ in 0..usize::MAX {}`) surfaces a clear error instead of
    /// hanging the compiler.
    fn eval_for(
        &mut self,
        pat_id: glyim_hir::PatId,
        iterable_id: ExprId,
        body_id: ExprId,
        span: Span,
        depth: u32,
    ) -> ConstEvalResult<ConstValue> {
        const MAX_ITERS: u32 = 1_000_000;
        let iterable_val = self.evaluate_at_depth(iterable_id, depth)?;

        // Materialize the element values from an already-const-evaluable
        // iterable. The element integer type is taken from the range's bound
        // (start if present, else end) so `for x in 0u8..4u8` yields `u8`s.
        let elements: Vec<ConstValue> = match iterable_val {
            ConstValue::Range(start, end, inclusive) => {
                let (s, e) = match (
                    start.as_deref().and_then(|c| c.as_i128()),
                    end.as_deref().and_then(|c| c.as_i128()),
                ) {
                    (Some(s), Some(e)) => (s, e),
                    _ => {
                        return Err(ConstEvalError::new(
                            "const `for` over a range requires concrete `start` and `end` bounds",
                            span,
                        ))
                    }
                };
                let proto = start
                    .as_deref()
                    .or(end.as_deref());
                let mk = |i: i128| -> ConstValue {
                    match proto {
                        Some(ConstValue::Int(_, ty)) => ConstValue::Int(i, *ty),
                        Some(ConstValue::Uint(_, ty)) => ConstValue::Uint(i as u128, *ty),
                        _ => ConstValue::Int(i, IntTy::I32),
                    }
                };
                let mut v = Vec::new();
                if inclusive {
                    let mut i = s;
                    while i <= e {
                        v.push(mk(i));
                        i = i.saturating_add(1);
                        if v.len() > MAX_ITERS as usize {
                            break;
                        }
                    }
                } else {
                    let mut i = s;
                    while i < e {
                        v.push(mk(i));
                        i = i.saturating_add(1);
                        if v.len() > MAX_ITERS as usize {
                            break;
                        }
                    }
                }
                v
            }
            ConstValue::Array(arr) => arr,
            ConstValue::Tuple(tup) => tup,
            other => {
                let _ = other;
                return Err(ConstEvalError::new(
                    "`for` over this iterable kind is not supported in const evaluation",
                    span,
                ))
            }
        };

        for elem in elements {
            // Bind the pattern into the *current* scope (the scope the loop
            // lives in), matching real `for` semantics where the loop variable
            // shares the enclosing scope. This is also what makes the
            // `acc = acc + x` accumulation idiom correct: assignments inside
            // the body must persist across iterations, so they must target the
            // same scope that holds the accumulator.
            self.pattern_matches(&pat_id, &elem)?;
            self.evaluate_at_depth(body_id, depth)?;
            match self.loop_control.take() {
                Some(LoopControl::Break) => return Ok(ConstValue::Unit),
                Some(LoopControl::Continue) => continue,
                None => continue,
            }
        }
        Ok(ConstValue::Unit)
    }

    fn eval_cast(
        &self,
        val: ConstValue,
        ty: &glyim_hir::TypeRef,
        span: Span,
    ) -> ConstEvalResult<ConstValue> {
        use glyim_core::primitives::{FloatTy, IntTy, UintTy};
        use glyim_hir::TypeRef;
        match ty {
            TypeRef::Path(path) => {
                if let Some(name) = path.as_name() {
                    let s = glyim_core::interner::Interner::default();
                    let n = s.resolve(name);
                    match n {
                        "i8" => val
                            .as_i128()
                            .map(|v| ConstValue::Int(v as i8 as i128, IntTy::I8))
                            .ok_or_else(|| ConstEvalError::new("cannot cast to i8", span)),
                        "i16" => val
                            .as_i128()
                            .map(|v| ConstValue::Int(v as i16 as i128, IntTy::I16))
                            .ok_or_else(|| ConstEvalError::new("cannot cast to i16", span)),
                        "i32" => val
                            .as_i128()
                            .map(|v| ConstValue::Int(v as i32 as i128, IntTy::I32))
                            .ok_or_else(|| ConstEvalError::new("cannot cast to i32", span)),
                        "i64" => val
                            .as_i128()
                            .map(|v| ConstValue::Int(v as i64 as i128, IntTy::I64))
                            .ok_or_else(|| ConstEvalError::new("cannot cast to i64", span)),
                        "u8" => val
                            .as_u128()
                            .map(|v| ConstValue::Uint(v as u8 as u128, UintTy::U8))
                            .ok_or_else(|| ConstEvalError::new("cannot cast to u8", span)),
                        "u16" => val
                            .as_u128()
                            .map(|v| ConstValue::Uint(v as u16 as u128, UintTy::U16))
                            .ok_or_else(|| ConstEvalError::new("cannot cast to u16", span)),
                        "u32" => val
                            .as_u128()
                            .map(|v| ConstValue::Uint(v as u32 as u128, UintTy::U32))
                            .ok_or_else(|| ConstEvalError::new("cannot cast to u32", span)),
                        "u64" => val
                            .as_u128()
                            .map(|v| ConstValue::Uint(v as u64 as u128, UintTy::U64))
                            .ok_or_else(|| ConstEvalError::new("cannot cast to u64", span)),
                        "f32" => val
                            .as_f64()
                            .map(|v| {
                                ConstValue::FloatBits((v as f32).to_bits() as u64, FloatTy::F32)
                            })
                            .ok_or_else(|| ConstEvalError::new("cannot cast to f32", span)),
                        "f64" => val
                            .as_f64()
                            .map(|v| ConstValue::FloatBits(v.to_bits(), FloatTy::F64))
                            .ok_or_else(|| ConstEvalError::new("cannot cast to f64", span)),
                        "bool" => Ok(ConstValue::Bool(
                            val.as_i128().map(|v| v != 0).unwrap_or(false),
                        )),
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
