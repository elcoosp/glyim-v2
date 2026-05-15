use glyim_core::{BinOp, CrateId, DefId, LocalDefId, UnOp};
use glyim_mir::*;
use glyim_type::TyCtx;
use std::collections::HashMap;

mod interp_error;
mod interp_value;

pub use interp_error::InterpError;
pub use interp_value::InterpValue;

pub struct Interpreter<'tcx> {
    tcx: &'tcx TyCtx,
    step_limit: usize,
    recursion_limit: usize,
    step_count: usize,
    recursion_depth: usize,
    function_table: HashMap<DefId, Body>,
    current_body: Option<Body>,
    current_bb: BasicBlockIdx,
    locals: Vec<Option<InterpValue>>,
    local_decls: Vec<LocalDecl>,
    call_stack: Vec<CallFrame>,
}

#[allow(dead_code)]
struct CallFrame {
    body: Body,
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
            local_decls: Vec::new(),
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
        self.local_decls = body.locals.iter().cloned().collect();
        self.call_stack.clear();
        self.step_count = 0;
        self.recursion_depth = 1;
        self.run_current_function()
    }

    fn run_current_function(&mut self) -> InterpResult<()> {
        let mut body = self.current_body.take().unwrap();
        let mut bb_idx = self.current_bb;

        loop {
            self.step_count += 1;
            if self.step_count > self.step_limit {
                self.current_body = Some(body);
                self.current_bb = bb_idx;
                return Err(InterpError::TimedOut);
            }

            let terminator_kind = body.basic_blocks[bb_idx].terminator.kind.clone();

            for stmt in &body.basic_blocks[bb_idx].statements {
                self.execute_statement(stmt)?;
            }

            match terminator_kind {
                TerminatorKind::Goto { target } => {
                    bb_idx = target;
                }
                TerminatorKind::SwitchInt {
                    discr,
                    switch_ty,
                    targets,
                } => {
                    let val = self.eval_operand(&discr)?;
                    let discr_u128 = if switch_ty == glyim_type::Ty::BOOL {
                        if let Ok(b) = self.interp_value_to_bool(&val) {
                            if b { 1u128 } else { 0u128 }
                        } else {
                            self.interp_value_to_u128(&val)
                        }
                    } else {
                        self.interp_value_to_u128(&val)
                    };
                    let mut next_bb = targets.otherwise();
                    for (v, bb) in targets.iter() {
                        if v == discr_u128 {
                            next_bb = bb;
                            break;
                        }
                    }
                    bb_idx = next_bb;
                }
                TerminatorKind::Return => {
                    if let Some(frame) = self.call_stack.pop() {
                        let ret_val = self.read_place(&Place::new(LocalIdx::from_raw(0)))?;
                        let caller_body = frame.body;
                        bb_idx = frame.target_bb;
                        self.locals = frame.locals;
                        self.local_decls = caller_body.locals.iter().cloned().collect();
                        self.write_place(&frame.return_place, ret_val)?;
                        self.recursion_depth -= 1;
                        body = caller_body;
                        continue;
                    } else {
                        self.current_body = Some(body);
                        self.current_bb = bb_idx;
                        return Ok(());
                    }
                }
                TerminatorKind::Unreachable => {
                    self.current_body = Some(body);
                    self.current_bb = bb_idx;
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
                        self.current_bb = bb_idx;
                        return Err(InterpError::StackOverflow);
                    }

                    let mut callee_locals = vec![None; callee_body.locals.len()];
                    for (i, val) in arg_values.into_iter().enumerate() {
                        callee_locals[i + 1] = Some(val);
                    }

                    let next_bb = target
                        .unwrap_or_else(|| BasicBlockIdx::from_raw((bb_idx.index() + 1) as u32));

                    let caller_frame = CallFrame {
                        body,
                        bb: next_bb,
                        locals: std::mem::take(&mut self.locals),
                        return_place: destination,
                        target_bb: next_bb,
                    };

                    self.call_stack.push(caller_frame);
                    self.local_decls = callee_body.locals.iter().cloned().collect();
                    self.locals = callee_locals;
                    body = callee_body;
                    bb_idx = BasicBlockIdx::from_raw(0);
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
                            self.current_bb = bb_idx;
                            return Err(InterpError::Panic(
                                "assert condition must be bool".to_string(),
                            ));
                        }
                    };
                    if is_true == expected {
                        bb_idx = target;
                    } else {
                        self.current_body = Some(body);
                        self.current_bb = bb_idx;
                        return Err(InterpError::Panic(format!("assert failed: {:?}", msg)));
                    }
                }
                TerminatorKind::Drop {
                    place: _,
                    target,
                    cleanup: _,
                } => {
                    bb_idx = target;
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
                let (left, right) = operands.as_ref();
                let l = self.eval_operand(left)?;
                let r = self.eval_operand(right)?;
                self.eval_binary_op(*op, &l, &r)
            }
            Rvalue::UnaryOp(op, operand) => {
                let v = self.eval_operand(operand)?;
                self.eval_unary_op(*op, &v)
            }
            Rvalue::Ref(place, _borrow_kind) => {
                let local_idx = place.local.index();
                Ok(InterpValue::Ref(local_idx))
            }
            Rvalue::Aggregate(_kind, operands) => {
                let mut values = Vec::with_capacity(operands.len());
                for op in operands {
                    values.push(self.eval_operand(op)?);
                }
                if values.is_empty() {
                    Ok(InterpValue::Unit)
                } else {
                    Ok(InterpValue::Aggregate(values))
                }
            }
            Rvalue::Discriminant(place) => {
                let val = self.read_place(place)?;
                match &val {
                    InterpValue::Aggregate(fields) => {
                        if fields.is_empty() {
                            Ok(InterpValue::Int(0))
                        } else {
                            Ok(fields[0].clone())
                        }
                    }
                    _ => {
                        tracing::warn!("STUB: Discriminant on non-aggregate, returning 0");
                        Ok(InterpValue::Int(0))
                    }
                }
            }
            Rvalue::Len(place) => {
                let local_decl = &self.local_decls[place.local.index()];
                let len = self.array_length_from_ty(&local_decl.ty)?;
                Ok(InterpValue::Int(len as i128))
            }
            Rvalue::Cast(kind, operand, _target_ty) => {
                let val = self.eval_operand(operand)?;
                match kind {
                    CastKind::IntToInt => Ok(val),
                    CastKind::IntToFloat => match val {
                        InterpValue::Int(i) => Ok(InterpValue::Int(i)),
                        _ => Err(InterpError::Panic("expected int for IntToFloat cast".into())),
                    },
                    _ => {
                        tracing::warn!("STUB: Cast kind {:?} not implemented", kind);
                        Err(InterpError::Panic(format!(
                            "Cast kind {:?} not implemented",
                            kind
                        )))
                    }
                }
            }
            Rvalue::Repeat(operand, _count) => {
                let val = self.eval_operand(operand)?;
                tracing::warn!("STUB: Repeat rvalue - returning single value instead of array");
                Ok(val)
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
            MirConstKind::Char(ch) => Ok(InterpValue::Int(*ch as i128)),
            MirConstKind::FloatBits(_) => {
                tracing::warn!("STUB: FloatBits const not implemented");
                Err(InterpError::Panic("FloatBits const not implemented".into()))
            }
            MirConstKind::String(_) => {
                tracing::warn!("STUB: String const not implemented");
                Err(InterpError::Panic("String const not implemented".into()))
            }
            MirConstKind::FnRef(_) => {
                tracing::warn!("STUB: FnRef constant used as value? Not supported");
                Err(InterpError::Panic(
                    "FnRef constant not supported as value".into(),
                ))
            }
            MirConstKind::Error => Err(InterpError::Panic("Error const encountered".into())),
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
                BinOp::And => Ok(InterpValue::Bool(*l && *r)),
                BinOp::Or => Ok(InterpValue::Bool(*l || *r)),
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
        let idx = place.local.index();
        let mut val = self
            .locals
            .get(idx)
            .and_then(|opt| opt.as_ref())
            .cloned()
            .ok_or_else(|| InterpError::Panic(format!("read from uninitialized local {}", idx)))?;

        for proj in place.projection.iter() {
            match proj {
                ProjectionElem::Deref => {
                    tracing::warn!("STUB: Deref projection not implemented");
                    return Err(InterpError::Panic("Deref projection not implemented".into()));
                }
                ProjectionElem::Field(field_idx) => {
                    match &val {
                        InterpValue::Aggregate(fields) => {
                            let idx = field_idx.index();
                            val = fields
                                .get(idx)
                                .cloned()
                                .ok_or_else(|| {
                                    InterpError::Panic(format!(
                                        "field index {} out of bounds (len {})",
                                        idx,
                                        fields.len()
                                    ))
                                })?;
                        }
                        _ => {
                            return Err(InterpError::Panic(format!(
                                "field projection on non-aggregate value: {:?}",
                                val
                            )));
                        }
                    }
                }
                ProjectionElem::Index(local) => {
                    let index_val = self
                        .locals
                        .get(local.index())
                        .and_then(|opt| opt.as_ref())
                        .ok_or_else(|| {
                            InterpError::Panic(format!(
                                "index local {} not initialized",
                                local.index()
                            ))
                        })?;
                    let index = match index_val {
                        InterpValue::Int(i) => *i as usize,
                        _ => {
                            return Err(InterpError::Panic(
                                "index must be an integer".into(),
                            ));
                        }
                    };
                    match &val {
                        InterpValue::Aggregate(elems) => {
                            val = elems.get(index).cloned().ok_or_else(|| {
                                InterpError::Panic(format!(
                                    "index {} out of bounds (len {})",
                                    index,
                                    elems.len()
                                ))
                            })?;
                        }
                        _ => {
                            return Err(InterpError::Panic(format!(
                                "index projection on non-aggregate value: {:?}",
                                val
                            )));
                        }
                    }
                }
                ProjectionElem::Downcast(_) => {
                    tracing::warn!("STUB: Downcast projection not implemented");
                    return Err(InterpError::Panic(
                        "Downcast projection not implemented".into(),
                    ));
                }
            }
        }
        Ok(val)
    }

    fn write_place(&mut self, place: &Place, val: InterpValue) -> InterpResult<()> {
        let idx = place.local.index();
        if idx >= self.locals.len() {
            return Err(InterpError::Panic(format!(
                "local index out of bounds: {}",
                idx
            )));
        }
        if place.projection.is_empty() {
            self.locals[idx] = Some(val);
            return Ok(());
        }
        // Handle projections: read the base, modify it, write back
        let base_place = Place {
            local: place.local,
            projection: Box::new([]),
        };
        let mut base_val = self
            .locals
            .get(idx)
            .and_then(|opt| opt.as_ref())
            .cloned()
            .ok_or_else(|| InterpError::Panic(format!("write to uninitialized local {}", idx)))?;

        let mut current = &mut base_val;
        let proj_len = place.projection.len();
        for (i, proj) in place.projection.iter().enumerate() {
            if i == proj_len - 1 {
                // Last projection: write the value
                match proj {
                    ProjectionElem::Field(field_idx) => match current {
                        InterpValue::Aggregate(fields) => {
                            let fi = field_idx.index();
                            if fi >= fields.len() {
                                return Err(InterpError::Panic(format!(
                                    "field index {} out of bounds (len {})",
                                    fi,
                                    fields.len()
                                )));
                            }
                            fields[fi] = val;
                            self.locals[idx] = Some(base_val);
                            return Ok(());
                        }
                        _ => {
                            return Err(InterpError::Panic(
                                "field projection on non-aggregate".into(),
                            ));
                        }
                    },
                    ProjectionElem::Index(index_local) => {
                        let index_val = self
                            .locals
                            .get(index_local.index())
                            .and_then(|opt| opt.as_ref())
                            .ok_or_else(|| {
                                InterpError::Panic(format!(
                                    "index local {} not initialized",
                                    index_local.index()
                                ))
                            })?;
                        let index = match index_val {
                            InterpValue::Int(i) => *i as usize,
                            _ => {
                                return Err(InterpError::Panic(
                                    "index must be an integer".into(),
                                ));
                            }
                        };
                        match current {
                            InterpValue::Aggregate(elems) => {
                                if index >= elems.len() {
                                    return Err(InterpError::Panic(format!(
                                        "index {} out of bounds (len {})",
                                        index,
                                        elems.len()
                                    )));
                                }
                                elems[index] = val;
                                self.locals[idx] = Some(base_val);
                                return Ok(());
                            }
                            _ => {
                                return Err(InterpError::Panic(
                                    "index projection on non-aggregate".into(),
                                ));
                            }
                        }
                    },
                    _ => {
                        return Err(InterpError::Panic(
                            "write with unsupported projection".into(),
                        ));
                    }
                }
            } else {
                // Intermediate projection: traverse down
                match proj {
                    ProjectionElem::Field(field_idx) => {
                        match current {
                            InterpValue::Aggregate(fields) => {
                                let fi = field_idx.index();
                                // We need to get a mutable reference to the inner field.
                                // Since we own base_val, we'll reconstruct.
                                let inner = fields.get(fi).cloned().ok_or_else(|| {
                                    InterpError::Panic(format!(
                                        "field index {} out of bounds",
                                        fi
                                    ))
                                })?;
                                fields[fi] = inner;
                            }
                            _ => {
                                return Err(InterpError::Panic(
                                    "field projection on non-aggregate".into(),
                                ));
                            }
                        }
                    }
                    ProjectionElem::Index(index_local) => {
                        let index_val = self
                            .locals
                            .get(index_local.index())
                            .and_then(|opt| opt.as_ref())
                            .ok_or_else(|| {
                                InterpError::Panic(format!(
                                    "index local {} not initialized",
                                    index_local.index()
                                ))
                            })?;
                        let index = match index_val {
                            InterpValue::Int(i) => *i as usize,
                            _ => return Err(InterpError::Panic("index must be an integer".into())),
                        };
                        match current {
                            InterpValue::Aggregate(elems) => {
                                let inner = elems.get(index).cloned().ok_or_else(|| {
                                    InterpError::Panic(format!("index {} out of bounds", index))
                                })?;
                                elems[index] = inner;
                            }
                            _ => {
                                return Err(InterpError::Panic(
                                    "index projection on non-aggregate".into(),
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(InterpError::Panic(
                            "intermediate projection not supported".into(),
                        ));
                    }
                }
                return Err(InterpError::Panic(
                    "nested projections not fully implemented".into(),
                ));
            }
        }
        Ok(())
    }

    fn resolve_callee(&self, func: &Operand) -> InterpResult<DefId> {
        match func {
            Operand::Constant(c) => match &c.kind {
                MirConstKind::FnRef(def_id) => Ok(*def_id),
                MirConstKind::Int(id) => Ok(DefId::new(
                    CrateId::from_raw(0),
                    LocalDefId::from_raw(*id as u32),
                )),
                _ => Err(InterpError::Panic(
                    "callee must be a function reference".into(),
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
            InterpValue::Aggregate(fields) => {
                if fields.is_empty() {
                    0
                } else {
                    self.interp_value_to_u128(&fields[0])
                }
            }
            InterpValue::Ref(idx) => *idx as u128,
        }
    }

    fn interp_value_to_bool(&self, val: &InterpValue) -> InterpResult<bool> {
        match val {
            InterpValue::Bool(b) => Ok(*b),
            InterpValue::Int(i) => Ok(*i != 0),
            InterpValue::Unit => Ok(false),
            InterpValue::Aggregate(fields) => {
                if fields.is_empty() {
                    Ok(false)
                } else {
                    self.interp_value_to_bool(&fields[0])
                }
            }
            InterpValue::Ref(_) => Ok(true),
        }
    }

    fn array_length_from_ty(&self, ty: &glyim_type::Ty) -> InterpResult<usize> {
        let kind = self.tcx.ty_kind(*ty);
        match kind {
            glyim_type::TyKind::Array(_, const_val) => match &const_val.kind {
                glyim_type::ConstKind::Int(n) => Ok(*n as usize),
                glyim_type::ConstKind::Uint(n) => Ok(*n as usize),
                _ => {
                    tracing::warn!("STUB: Len for non-constant array length");
                    Err(InterpError::Panic(
                        "Len: unsupported array length kind".into(),
                    ))
                }
            },
            _ => Err(InterpError::Panic("Len: expected array type".into())),
        }
    }

    pub fn get_local_value(&self, local: LocalIdx) -> Option<&InterpValue> {
        self.locals.get(local.index())?.as_ref()
    }
}

pub type InterpResult<T> = Result<T, InterpError>;

#[cfg(test)]
mod tests;
