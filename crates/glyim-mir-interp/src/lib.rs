use glyim_core::{BinOp, CrateId, DefId, LocalDefId, UnOp};
use glyim_mir::*;
use glyim_type::TyCtx;
use std::collections::HashMap;

mod interp_error;
mod interp_value;

pub use interp_error::InterpError;
pub use interp_value::InterpValue;

pub struct Interpreter<'tcx> {
    #[allow(dead_code)]
    tcx: &'tcx TyCtx,
    step_limit: usize,
    recursion_limit: usize,
    step_count: usize,
    recursion_depth: usize,
    function_table: HashMap<DefId, Body>,
    current_body: Option<Body>,
    current_bb: BasicBlockIdx,
    locals: Vec<Option<InterpValue>>,
    call_stack: Vec<CallFrame>,
}

struct CallFrame {
    body: Body,
    #[allow(dead_code)]
    bb: BasicBlockIdx,
    locals: Vec<Option<InterpValue>>,
    return_place: Place,
    target_bb: BasicBlockIdx,
}

impl<'tcx> Interpreter<'tcx> {
    pub fn new(tcx: &'tcx TyCtx) -> Self {
        Interpreter {
            tcx,
            step_limit: 1_000_000,
            recursion_limit: 256,
            step_count: 0,
            recursion_depth: 0,
            function_table: HashMap::new(),
            current_body: None,
            current_bb: BasicBlockIdx::from_raw(0),
            locals: Vec::new(),
            call_stack: Vec::new(),
        }
    }

    pub fn with_step_limit(mut self, limit: usize) -> Self {
        self.step_limit = limit;
        self
    }

    pub fn with_recursion_limit(mut self, limit: usize) -> Self {
        self.recursion_limit = limit;
        self
    }

    pub fn add_function(&mut self, def_id: DefId, body: Body) {
        self.function_table.insert(def_id, body);
    }

    pub fn step_limit(&self) -> usize {
        self.step_limit
    }

    pub fn recursion_limit(&self) -> usize {
        self.recursion_limit
    }

    pub fn run_body(&mut self, body: &Body) -> InterpResult<()> {
        self.current_body = Some(body.clone());
        self.current_bb = BasicBlockIdx::from_raw(0);
        self.locals = vec![None; body.locals.len()];
        self.call_stack.clear();
        self.step_count = 0;
        self.recursion_depth = 1;
        self.run_current_function()
    }

    fn run_current_function(&mut self) -> InterpResult<()> {
        // Take ownership of the current body so we don't borrow self
        let mut body = self.current_body.take().unwrap();
        loop {
            self.step_count += 1;
            if self.step_count > self.step_limit {
                // Put body back before returning
                self.current_body = Some(body);
                return Err(InterpError::TimedOut);
            }

            // Clone the current basic block data to avoid borrowing body across mutable calls
            let bb_data = body.basic_blocks[self.current_bb].clone();

            // Execute statements (this only needs &mut self, not body)
            for stmt in &bb_data.statements {
                self.execute_statement(stmt)?;
            }

            // Execute terminator
            match bb_data.terminator.kind {
                TerminatorKind::Goto { target } => {
                    self.current_bb = target;
                }
                TerminatorKind::SwitchInt {
                    discr,
                    switch_ty: _,
                    targets,
                } => {
                    let val = self.eval_operand(&discr)?;
                    let discr_u128 = self.interp_value_to_u128(&val);
                    let mut next_bb = targets.otherwise();
                    for (v, bb) in targets.iter() {
                        if v == discr_u128 {
                            next_bb = bb;
                            break;
                        }
                    }
                    self.current_bb = next_bb;
                }
                TerminatorKind::Return => {
                    if let Some(frame) = self.call_stack.pop() {
                        let ret_val = self.read_place(&Place::new(LocalIdx::from_raw(0)))?;
                        // Restore caller state
                        body = frame.body;
                        self.current_bb = frame.target_bb;
                        self.locals = frame.locals;
                        self.write_place(&frame.return_place, ret_val)?;
                        self.recursion_depth -= 1;
                    } else {
                        self.current_body = Some(body);
                        return Ok(());
                    }
                }
                TerminatorKind::Unreachable => {
                    self.current_body = Some(body);
                    return Err(InterpError::Panic(
                        "reached unreachable terminator".to_string(),
                    ));
                }
                TerminatorKind::Call {
                    func,
                    args,
                    destination,
                    target,
                    cleanup: _,
                } => {
                    let callee_id = self.resolve_callee(&func)?;
                    let callee_body =
                        self.function_table
                            .get(&callee_id)
                            .cloned()
                            .ok_or_else(|| {
                                InterpError::Panic(format!("function not found: {:?}", callee_id))
                            })?;

                    let mut arg_values = Vec::new();
                    for arg_op in &args {
                        arg_values.push(self.eval_operand(arg_op)?);
                    }

                    self.recursion_depth += 1;
                    if self.recursion_depth > self.recursion_limit {
                        self.current_body = Some(body);
                        return Err(InterpError::StackOverflow);
                    }

                    let mut callee_locals = vec![None; callee_body.locals.len()];
                    for (i, val) in arg_values.into_iter().enumerate() {
                        callee_locals[i + 1] = Some(val);
                    }

                    let next_bb = target.unwrap_or_else(|| {
                        BasicBlockIdx::from_raw((self.current_bb.index() + 1) as u32)
                    });

                    let caller_frame = CallFrame {
                        body,
                        bb: next_bb,
                        locals: std::mem::take(&mut self.locals),
                        return_place: destination,
                        target_bb: next_bb,
                    };

                    self.call_stack.push(caller_frame);
                    body = callee_body;
                    self.current_bb = BasicBlockIdx::from_raw(0);
                    self.locals = callee_locals;
                }
                TerminatorKind::Assert {
                    cond,
                    expected,
                    target,
                    cleanup: _,
                    msg,
                } => {
                    let val = self.eval_operand(&cond)?;
                    let is_true = match val {
                        InterpValue::Bool(b) => b,
                        _ => {
                            self.current_body = Some(body);
                            return Err(InterpError::Panic(
                                "assert condition must be bool".to_string(),
                            ));
                        }
                    };
                    if is_true == expected {
                        self.current_bb = target;
                    } else {
                        self.current_body = Some(body);
                        return Err(InterpError::Panic(format!("assert failed: {:?}", msg)));
                    }
                }
                TerminatorKind::Drop {
                    place: _,
                    target,
                    cleanup: _,
                } => {
                    self.current_bb = target;
                }
            }
        }
    }

    fn execute_statement(&mut self, stmt: &Statement) -> InterpResult<()> {
        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                let val = self.eval_rvalue(rvalue)?;
                self.write_place(place, val)?;
            }
            StatementKind::StorageLive(local) => {
                self.locals[local.index()] = None;
            }
            StatementKind::StorageDead(local) => {
                self.locals[local.index()] = None;
            }
            StatementKind::Nop => {}
        }
        Ok(())
    }

    fn eval_rvalue(&self, rvalue: &Rvalue) -> InterpResult<InterpValue> {
        match rvalue {
            Rvalue::Use(operand) => self.eval_operand(operand),
            Rvalue::BinaryOp(op, operands) => {
                let (ref left, ref right) = **operands;
                let l = self.eval_operand(left)?;
                let r = self.eval_operand(right)?;
                self.eval_binary_op(*op, &l, &r)
            }
            Rvalue::UnaryOp(op, operand) => {
                let v = self.eval_operand(operand)?;
                self.eval_unary_op(*op, &v)
            }
            Rvalue::Ref(_, _) => {
                tracing::warn!("STUB: Ref rvalue not implemented");
                Err(InterpError::Panic("Ref rvalue not implemented".into()))
            }
            Rvalue::Aggregate(_, _) => {
                tracing::warn!("STUB: Aggregate rvalue not implemented");
                Err(InterpError::Panic(
                    "Aggregate rvalue not implemented".into(),
                ))
            }
            Rvalue::Discriminant(_) => {
                tracing::warn!("STUB: Discriminant rvalue not implemented");
                Err(InterpError::Panic(
                    "Discriminant rvalue not implemented".into(),
                ))
            }
            Rvalue::Len(_) => {
                tracing::warn!("STUB: Len rvalue not implemented");
                Err(InterpError::Panic("Len rvalue not implemented".into()))
            }
            Rvalue::Cast(_, _, _) => {
                tracing::warn!("STUB: Cast rvalue not implemented");
                Err(InterpError::Panic("Cast rvalue not implemented".into()))
            }
            Rvalue::Repeat(_, _) => {
                tracing::warn!("STUB: Repeat rvalue not implemented");
                Err(InterpError::Panic("Repeat rvalue not implemented".into()))
            }
        }
    }

    fn eval_operand(&self, operand: &Operand) -> InterpResult<InterpValue> {
        match operand {
            Operand::Constant(c) => self.eval_mir_const(c),
            Operand::Copy(place) | Operand::Move(place) => self.read_place(place),
        }
    }

    fn eval_mir_const(&self, c: &MirConst) -> InterpResult<InterpValue> {
        match &c.kind {
            MirConstKind::Int(v) => Ok(InterpValue::Int(*v)),
            MirConstKind::Uint(v) => Ok(InterpValue::Int(*v as i128)),
            MirConstKind::Bool(v) => Ok(InterpValue::Bool(*v)),
            MirConstKind::Unit => Ok(InterpValue::Unit),
            _ => {
                tracing::warn!("STUB: unsupported const kind: {:?}", c.kind);
                Err(InterpError::Panic(format!(
                    "unsupported const kind: {:?}",
                    c.kind
                )))
            }
        }
    }

    fn eval_binary_op(
        &self,
        op: BinOp,
        left: &InterpValue,
        right: &InterpValue,
    ) -> InterpResult<InterpValue> {
        match (left, right) {
            (InterpValue::Int(l), InterpValue::Int(r)) => {
                let result = match op {
                    BinOp::Add => l.wrapping_add(*r),
                    BinOp::Sub => l.wrapping_sub(*r),
                    BinOp::Mul => l.wrapping_mul(*r),
                    BinOp::Div => {
                        if *r == 0 {
                            return Err(InterpError::Panic("division by zero".into()));
                        }
                        l.wrapping_div(*r)
                    }
                    BinOp::Rem => {
                        if *r == 0 {
                            return Err(InterpError::Panic("remainder by zero".into()));
                        }
                        l.wrapping_rem(*r)
                    }
                    BinOp::BitAnd => l & *r,
                    BinOp::BitOr => l | *r,
                    BinOp::BitXor => l ^ *r,
                    BinOp::Shl => l.wrapping_shl(*r as u32),
                    BinOp::Shr => l.wrapping_shr(*r as u32),
                    BinOp::Eq => return Ok(InterpValue::Bool(l == r)),
                    BinOp::Ne => return Ok(InterpValue::Bool(l != r)),
                    BinOp::Lt => return Ok(InterpValue::Bool(l < r)),
                    BinOp::Gt => return Ok(InterpValue::Bool(l > r)),
                    BinOp::LtEq => return Ok(InterpValue::Bool(l <= r)),
                    BinOp::GtEq => return Ok(InterpValue::Bool(l >= r)),
                    _ => {
                        return Err(InterpError::Panic(format!(
                            "unsupported integer binop: {:?}",
                            op
                        )));
                    }
                };
                Ok(InterpValue::Int(result))
            }
            (InterpValue::Bool(l), InterpValue::Bool(r)) => match op {
                BinOp::Eq => Ok(InterpValue::Bool(l == r)),
                BinOp::Ne => Ok(InterpValue::Bool(l != r)),
                _ => Err(InterpError::Panic(format!(
                    "unsupported bool binop: {:?}",
                    op
                ))),
            },
            _ => Err(InterpError::Panic(format!(
                "unsupported binop types: {:?}",
                op
            ))),
        }
    }

    fn eval_unary_op(&self, op: UnOp, val: &InterpValue) -> InterpResult<InterpValue> {
        match (op, val) {
            (UnOp::Not, InterpValue::Bool(b)) => Ok(InterpValue::Bool(!b)),
            (UnOp::Neg, InterpValue::Int(i)) => Ok(InterpValue::Int(-i)),
            _ => Err(InterpError::Panic(format!(
                "unsupported unary op: {:?}",
                op
            ))),
        }
    }

    fn read_place(&self, place: &Place) -> InterpResult<InterpValue> {
        if !place.projection.is_empty() {
            return Err(InterpError::Panic("projections not implemented".into()));
        }
        let idx = place.local.index();
        self.locals
            .get(idx)
            .and_then(|opt| opt.as_ref())
            .cloned()
            .ok_or_else(|| InterpError::Panic(format!("read from uninitialized local {}", idx)))
    }

    fn write_place(&mut self, place: &Place, val: InterpValue) -> InterpResult<()> {
        if !place.projection.is_empty() {
            return Err(InterpError::Panic("projections not implemented".into()));
        }
        let idx = place.local.index();
        if idx >= self.locals.len() {
            return Err(InterpError::Panic(format!(
                "local index out of bounds: {}",
                idx
            )));
        }
        self.locals[idx] = Some(val);
        Ok(())
    }

    fn resolve_callee(&self, func: &Operand) -> InterpResult<DefId> {
        match func {
            Operand::Constant(c) => match &c.kind {
                MirConstKind::Int(id) => Ok(DefId::new(
                    CrateId::from_raw(0),
                    LocalDefId::from_raw(*id as u32),
                )),
                _ => Err(InterpError::Panic(
                    "callee must be constant int encoding DefId".into(),
                )),
            },
            _ => Err(InterpError::Panic(
                "indirect function calls not implemented".into(),
            )),
        }
    }

    fn interp_value_to_u128(&self, val: &InterpValue) -> u128 {
        match val {
            InterpValue::Int(i) => *i as u128,
            InterpValue::Bool(b) => *b as u128,
            InterpValue::Unit => 0,
        }
    }

    pub fn get_local_value(&self, local: LocalIdx) -> Option<&InterpValue> {
        self.locals.get(local.index())?.as_ref()
    }
}

pub type InterpResult<T> = Result<T, InterpError>;

#[cfg(test)]
mod tests;
