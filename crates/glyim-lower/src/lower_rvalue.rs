use tracing;n
use tracing;n
use tracing;n
use crate::builder::{LoopInfo, MirBuilder};
use crate::lower_terminator::TerminatorExt;
use glyim_core::primitives::Mutability;
use glyim_diag::GlyimDiagnostic;
use glyim_mir::{self, BasicBlockIdx, CastKind, LocalIdx, ProjectionElem};
use glyim_type::*;
use glyim_typeck::thir;

impl<'a> MirBuilder<'a> {
    // ---- Statement lowering ----

    pub fn lower_stmt(&mut self, stmt: &thir::Stmt) {
        match stmt {
            thir::Stmt::Let {
                name,
                ty,
                init,
                span,
                pat,
            } => {
                let init_local = if let Some(init_expr) = init {
                    let temp_local = self.alloc_local(*ty, Mutability::Mut, *span);
                    self.push_stmt(glyim_mir::StatementKind::StorageLive(temp_local), *span);
                    let rvalue = self.lower_expr_to_rvalue(init_expr);
                    self.push_stmt(
                        glyim_mir::StatementKind::Assign(glyim_mir::Place::new(temp_local), rvalue),
                        *span,
                    );
                    Some(temp_local)
                } else {
                    None
                };
                self.bind_pattern(pat, init_local, *span);
                if let thir::PatternKind::Binding {
                    name: bind_name, ..
                } = &pat.kind
                {
                    if !self.var_map.contains_key(bind_name)
                        && let Some(local) = init_local
                    {
                        self.var_map.insert(*bind_name, local);
                    }
                } else if !self.var_map.contains_key(name)
                    && let Some(local) = init_local
                {
                    self.var_map.insert(*name, local);
                }
            }
            thir::Stmt::Assign { lhs, rhs, span } => {
                let place = self.lower_expr_to_place(lhs);
                let rvalue = self.lower_expr_to_rvalue(rhs);
                self.push_stmt(glyim_mir::StatementKind::Assign(place, rvalue), *span);
            }
            thir::Stmt::Return { value, span } => {
                if let Some(val_expr) = value {
                    let rvalue = self.lower_expr_to_rvalue(val_expr);
                    let ret_place = glyim_mir::Place::new(LocalIdx::from_raw(0));
                    self.push_stmt(glyim_mir::StatementKind::Assign(ret_place, rvalue), *span);
                }
                self.terminate(glyim_mir::TerminatorKind::Return, *span);
            }
            thir::Stmt::Expr { expr } => {
                let rvalue = self.lower_expr_to_rvalue(expr);
                // Assign to a temp so side effects are captured in MIR.
                // Expression statements may involve function calls, borrows, etc.
                let temp = self.alloc_local(expr.ty, Mutability::Mut, expr.span);
                self.push_stmt(glyim_mir::StatementKind::StorageLive(temp), expr.span);
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(glyim_mir::Place::new(temp), rvalue),
                    expr.span,
                );
            }
        }
    }

    // ---- Expression → Rvalue lowering ----

    pub fn lower_expr_to_rvalue(&mut self, expr: &thir::Expr) -> glyim_mir::Rvalue {
        match &expr.kind {
            thir::ExprKind::Literal(lit) => {
                let mir_const = self.lower_literal(lit, expr.ty, expr.span);
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(mir_const))
            }
            thir::ExprKind::VarRef(var_id) => {
                let local = LocalIdx::from_raw(var_id.to_raw());
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(glyim_mir::Place::new(local)))
            }
            thir::ExprKind::FnRef(_def_id) => {
                let (fn_def_id, substs) = match self.ctx.ty_ctx().ty_kind(expr.ty) {
                    TyKind::FnDef(id, sub) => (id, sub),
                    _ => {
                        tracing::warn!("FnRef with non-FnDef type, emitting Error constant");
                        return glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(
                            glyim_mir::MirConst {
                                kind: glyim_mir::MirConstKind::Error,
                                ty: expr.ty,
                                span: expr.span,
                            },
                        ));
                    }
                };
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Fn(*fn_def_id, *substs),
                    ty: expr.ty,
                    span: expr.span,
                }))
            }
            thir::ExprKind::Binary { op, lhs, rhs } => {
                let lhs_op = self.lower_expr_to_operand(lhs);
                let rhs_op = self.lower_expr_to_operand(rhs);
                glyim_mir::Rvalue::BinaryOp(*op, Box::new((lhs_op, rhs_op)))
            }
            thir::ExprKind::Unary { op, operand } => {
                let op_val = self.lower_expr_to_operand(operand);
                glyim_mir::Rvalue::UnaryOp(*op, op_val)
            }
            thir::ExprKind::Ref {
                mutability,
                operand,
            } => {
                let place = self.lower_expr_to_place(operand);
                let borrow_kind = match mutability {
                    Mutability::Mut => glyim_mir::BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                    Mutability::Not => glyim_mir::BorrowKind::Shared,
                };
                glyim_mir::Rvalue::Ref(place, borrow_kind)
            }
            thir::ExprKind::Call { func, args } => {
                let mut mir_args = Vec::new();
                for arg in args {
                    mir_args.push(self.lower_expr_to_operand(arg));
                }
                let func_op = self.lower_expr_to_operand(func);

                let dest_local = self.alloc_local(expr.ty, Mutability::Mut, expr.span);
                let dest_place = glyim_mir::Place::new(dest_local);

                let next_bb = self.new_block();
                self.terminate(
                    glyim_mir::TerminatorKind::Call {
                        func: func_op,
                        args: mir_args,
                        destination: dest_place.clone(),
                        target: Some(next_bb),
                        cleanup: None,
                    },
                    expr.span,
                );

                self.current_block = Some(next_bb);
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Move(dest_place))
            }
            thir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_op = self.lower_expr_to_operand(cond);

                let then_bb = self.new_block();
                let else_bb = self.new_block();
                let merge_bb = self.new_block();

                let dest_local = self.alloc_local(expr.ty, Mutability::Mut, expr.span);
                let dest_place = glyim_mir::Place::new(dest_local);

                let targets = glyim_mir::SwitchTargets::new(Box::new([(1, then_bb)]), else_bb);

                self.terminate(
                    glyim_mir::TerminatorKind::SwitchInt {
                        discr: cond_op,
                        switch_ty: cond.ty,
                        targets,
                    },
                    expr.span,
                );

                self.current_block = Some(then_bb);
                let then_val = self.lower_expr_to_rvalue(then_branch);
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(dest_place.clone(), then_val),
                    then_branch.span,
                );
                self.terminate(
                    glyim_mir::TerminatorKind::Goto { target: merge_bb },
                    then_branch.span,
                );

                self.current_block = Some(else_bb);
                if let Some(else_b) = else_branch {
                    let else_val = self.lower_expr_to_rvalue(else_b);
                    self.push_stmt(
                        glyim_mir::StatementKind::Assign(dest_place.clone(), else_val),
                        else_b.span,
                    );
                }
                self.terminate(
                    glyim_mir::TerminatorKind::Goto { target: merge_bb },
                    expr.span,
                );

                self.current_block = Some(merge_bb);
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Move(dest_place))
            }
            thir::ExprKind::Match { scrutinee, arms } => {
                self.lower_match(scrutinee, arms, expr.ty, expr.span)
            }
            thir::ExprKind::Block { stmts, tail } => {
                for stmt in stmts {
                    self.lower_stmt(stmt);
                    // If a statement (e.g. break/continue/return) terminated the block,
                    // skip remaining stmts and the tail — we're in dead code.
                    if self.current_block.is_none() {
                        return glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(
                            glyim_mir::MirConst {
                                kind: glyim_mir::MirConstKind::Unit,
                                ty: Ty::NEVER,
                                span: expr.span,
                            },
                        ));
                    }
                }
                if let Some(tail_expr) = tail {
                    self.lower_expr_to_rvalue(tail_expr)
                } else {
                    glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                        kind: glyim_mir::MirConstKind::Unit,
                        ty: Ty::UNIT,
                        span: expr.span,
                    }))
                }
            }
            thir::ExprKind::While { cond, body } => {
                let header_bb = self.new_block();
                let body_bb = self.new_block();
                let exit_bb = self.new_block();

                self.terminate(
                    glyim_mir::TerminatorKind::Goto { target: header_bb },
                    expr.span,
                );

                self.current_block = Some(header_bb);
                let cond_op = self.lower_expr_to_operand(cond);
                let targets = glyim_mir::SwitchTargets::new(Box::new([(1, body_bb)]), exit_bb);
                self.terminate(
                    glyim_mir::TerminatorKind::SwitchInt {
                        discr: cond_op,
                        switch_ty: cond.ty,
                        targets,
                    },
                    cond.span,
                );

                self.loop_stack.push(LoopInfo {
                    continue_bb: header_bb,
                    break_bb: exit_bb,
                });

                self.current_block = Some(body_bb);
                let _ = self.lower_expr_to_rvalue(body);

                self.loop_stack.pop();

                if self.current_block.is_some() {
                    self.terminate(
                        glyim_mir::TerminatorKind::Goto { target: header_bb },
                        body.span,
                    );
                }

                self.current_block = Some(exit_bb);
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Unit,
                    ty: Ty::UNIT,
                    span: expr.span,
                }))
            }
            thir::ExprKind::Loop { body } => {
                let loop_bb = self.new_block();
                let exit_bb = self.new_block();

                self.terminate(
                    glyim_mir::TerminatorKind::Goto { target: loop_bb },
                    expr.span,
                );

                self.loop_stack.push(LoopInfo {
                    continue_bb: loop_bb,
                    break_bb: exit_bb,
                });

                self.current_block = Some(loop_bb);
                let _ = self.lower_expr_to_rvalue(body);

                self.loop_stack.pop();

                if self.current_block.is_some() {
                    self.terminate(
                        glyim_mir::TerminatorKind::Goto { target: loop_bb },
                        body.span,
                    );
                }

                self.current_block = Some(exit_bb);
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Unit,
                    ty: Ty::NEVER,
                    span: expr.span,
                }))
            }
            thir::ExprKind::For {
                pat: _,
                iterable,
                body: _,
            } => {
                let _ = self.lower_expr_to_rvalue(iterable);
                tracing::warn!(
                    "STUB: for-loop lowering not fully implemented (requires iterator protocol)"
                );
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Unit,
                    ty: Ty::UNIT,
                    span: expr.span,
                }))
            }
            thir::ExprKind::Field {
                receiver,
                field,
                ty: _field_ty,
            } => {
                let base_place = self.lower_expr_to_place(receiver);
                let field_idx = self.resolve_field_index(receiver.ty, *field, expr.span);
                let field_idx = match field_idx {
                    Some(idx) => idx,
                    None => {
                        return glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(
                            glyim_mir::MirConst {
                                kind: glyim_mir::MirConstKind::Error,
                                ty: *_field_ty,
                                span: expr.span,
                            },
                        ));
                    }
                };
                let place =
                    self.place_with_projection(base_place, ProjectionElem::Field(field_idx));
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(place))
            }
            thir::ExprKind::Index { base, index } => {
                let base_place = self.lower_expr_to_place(base);
                let index_local = self.alloc_local(index.ty, Mutability::Not, index.span);
                let index_rval = self.lower_expr_to_rvalue(index);
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(
                        glyim_mir::Place::new(index_local),
                        index_rval,
                    ),
                    index.span,
                );
                let place =
                    self.place_with_projection(base_place, ProjectionElem::Index(index_local));
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(place))
            }
            thir::ExprKind::Cast { expr: inner } => {
                let operand = self.lower_expr_to_operand(inner);
                let inner_ty = inner.ty;
                let target_ty = expr.ty;
                let cast_kind = match (
                    self.ctx.ty_ctx().ty_kind(inner_ty),
                    self.ctx.ty_ctx().ty_kind(target_ty),
                ) {
                    (TyKind::Int(_), TyKind::Int(_)) => CastKind::IntToInt,
                    (TyKind::Float(_), TyKind::Int(_)) => CastKind::FloatToInt,
                    (TyKind::Int(_), TyKind::Float(_)) => CastKind::IntToFloat,
                    (TyKind::Float(_), TyKind::Float(_)) => CastKind::IntToFloat,
                    _ => CastKind::PtrToPtr,
                };
                glyim_mir::Rvalue::Cast(cast_kind, operand, target_ty)
            }
            thir::ExprKind::Tuple(elements) => {
                let mut mir_operands = Vec::new();
                for op_expr in elements {
                    mir_operands.push(self.lower_expr_to_operand(op_expr));
                }
                glyim_mir::Rvalue::Aggregate(glyim_mir::AggregateKind::Tuple, mir_operands)
            }
            thir::ExprKind::Array(elements) => {
                let elem_ty = match self.ctx.ty_ctx().ty_kind(expr.ty) {
                    TyKind::Slice(inner) | TyKind::Array(inner, _) => *inner,
                    _ => Ty::ERROR,
                };
                let mut mir_operands = Vec::new();
                for op_expr in elements {
                    mir_operands.push(self.lower_expr_to_operand(op_expr));
                }
                glyim_mir::Rvalue::Aggregate(glyim_mir::AggregateKind::Array(elem_ty), mir_operands)
            }
            thir::ExprKind::Struct {
                adt_id,
                variant_idx,
                fields,
                spread: _,
            } => {
                let substs = match self.ctx.ty_ctx().ty_kind(expr.ty) {
                    TyKind::Adt(_, substs) => substs,
                    _ => {
                        return glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(
                            glyim_mir::MirConst {
                                kind: glyim_mir::MirConstKind::Error,
                                ty: expr.ty,
                                span: expr.span,
                            },
                        ));
                    }
                };
                let mut mir_operands = Vec::new();
                for (_name, field_expr) in fields {
                    mir_operands.push(self.lower_expr_to_operand(field_expr));
                }
                let variant = glyim_mir::VariantIdx::from_raw(*variant_idx);
                glyim_mir::Rvalue::Aggregate(
                    glyim_mir::AggregateKind::Adt(*adt_id, variant, *substs),
                    mir_operands,
                )
            }
            thir::ExprKind::Break { value } => {
                if let Some(val_expr) = value {
                    let _ = self.lower_expr_to_rvalue(val_expr);
                }
                let target_bb = self.loop_stack.last().map(|info| info.break_bb);
                if let Some(target) = target_bb {
                    self.terminate(glyim_mir::TerminatorKind::Goto { target }, expr.span);
                } else {
                    self.diagnostics.push(GlyimDiagnostic::type_error(
                        expr.span,
                        "break outside of loop".to_string(),
                    ));
                    self.terminate(glyim_mir::TerminatorKind::Unreachable, expr.span);
                }
                // Do NOT create a new block — the current block is terminated.
                // The caller must check current_block before emitting statements.
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Unit,
                    ty: Ty::NEVER,
                    span: expr.span,
                }))
            }
            thir::ExprKind::Continue => {
                let target_bb = self.loop_stack.last().map(|info| info.continue_bb);
                if let Some(target) = target_bb {
                    self.terminate(glyim_mir::TerminatorKind::Goto { target }, expr.span);
                } else {
                    self.diagnostics.push(GlyimDiagnostic::type_error(
                        expr.span,
                        "continue outside of loop".to_string(),
                    ));
                    self.terminate(glyim_mir::TerminatorKind::Unreachable, expr.span);
                }
                // Do NOT create a new block — the current block is terminated.
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Unit,
                    ty: Ty::NEVER,
                    span: expr.span,
                }))
            }
            thir::ExprKind::Closure {
                body: _,
                captures: _,
            } => {
                tracing::warn!("STUB: closure lowering emits placeholder constant");
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Error,
                    ty: expr.ty,
                    span: expr.span,
                }))
            }
            thir::ExprKind::Err => {
                self.diagnostics.push(GlyimDiagnostic::new(
                    glyim_diag::ErrorCode {
                        category: glyim_diag::ErrorCategory::Internal,
                        number: 0,
                    },
                    glyim_diag::DiagSeverity::Warning,
                    "Err expression in THIR during lowering".to_string(),
                    glyim_diag::MultiSpan::from_span(expr.span),
                ));
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Error,
                    ty: expr.ty,
                    span: expr.span,
                }))
            }
        }
    }

    // ---- Expression → Operand lowering ----

    pub fn lower_expr_to_operand(&mut self, expr: &thir::Expr) -> glyim_mir::Operand {
        match &expr.kind {
            thir::ExprKind::Literal(_) | thir::ExprKind::FnRef(_) => {
                if let glyim_mir::Rvalue::Use(op) = self.lower_expr_to_rvalue(expr) {
                    op
                } else {
                    unreachable!("literal/FnRef always lowers to Rvalue::Use(Operand)")
                }
            }
            thir::ExprKind::VarRef(var_id) => {
                let local = LocalIdx::from_raw(var_id.to_raw());
                glyim_mir::Operand::Copy(glyim_mir::Place::new(local))
            }
            _ => {
                let rvalue = self.lower_expr_to_rvalue(expr);
                let local = self.alloc_local(expr.ty, Mutability::Mut, expr.span);
                let place = glyim_mir::Place::new(local);
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(place.clone(), rvalue),
                    expr.span,
                );
                glyim_mir::Operand::Move(place)
            }
        }
    }

    // ---- Expression → Place lowering ----

    pub fn lower_expr_to_place(&mut self, expr: &thir::Expr) -> glyim_mir::Place {
        match &expr.kind {
            thir::ExprKind::VarRef(var_id) => {
                let local = LocalIdx::from_raw(var_id.to_raw());
                glyim_mir::Place::new(local)
            }
            thir::ExprKind::Field {
                receiver,
                field,
                ty: _field_ty,
            } => {
                let base_place = self.lower_expr_to_place(receiver);
                let field_idx = self.resolve_field_index(receiver.ty, *field, expr.span);
                let field_idx = match field_idx {
                    Some(idx) => idx,
                    None => FieldIdx::from_raw(0),
                };
                self.place_with_projection(base_place, ProjectionElem::Field(field_idx))
            }
            thir::ExprKind::Index { base, index } => {
                let base_place = self.lower_expr_to_place(base);
                let index_local = self.alloc_local(index.ty, Mutability::Not, index.span);
                let index_rval = self.lower_expr_to_rvalue(index);
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(
                        glyim_mir::Place::new(index_local),
                        index_rval,
                    ),
                    index.span,
                );
                self.place_with_projection(base_place, ProjectionElem::Index(index_local))
            }
            thir::ExprKind::Ref {
                operand,
                mutability: _,
            } => self.lower_expr_to_place(operand),
            _ => {
                let rvalue = self.lower_expr_to_rvalue(expr);
                let local = self.alloc_local(expr.ty, Mutability::Mut, expr.span);
                let place = glyim_mir::Place::new(local);
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(place.clone(), rvalue),
                    expr.span,
                );
                place
            }
        }
    }

    // ---- Pattern binding ----

    pub fn bind_pattern(
        &mut self,
        pat: &thir::Pattern,
        init_local: Option<LocalIdx>,
        span: glyim_span::Span,
    ) {
        match &pat.kind {
            thir::PatternKind::Binding {
                name,
                mutability,
                subpattern,
            } => {
                let local = self.alloc_local(pat.ty, *mutability, span);
                self.var_map.insert(*name, local);
                self.push_stmt(glyim_mir::StatementKind::StorageLive(local), span);
                if let Some(init) = init_local {
                    let place = glyim_mir::Place::new(local);
                    let rvalue = glyim_mir::Rvalue::Use(glyim_mir::Operand::Move(
                        glyim_mir::Place::new(init),
                    ));
                    self.push_stmt(glyim_mir::StatementKind::Assign(place, rvalue), span);
                }
                if let Some(sub) = subpattern {
                    self.bind_pattern(sub, Some(local), span);
                }
            }
            thir::PatternKind::Wild => {}
            thir::PatternKind::Tuple(fields) => {
                if let Some(init) = init_local {
                    let init_place = glyim_mir::Place::new(init);
                    for (idx, field_pat) in fields.iter().enumerate() {
                        let field_proj = ProjectionElem::Field(FieldIdx::from_raw(idx as u32));
                        let field_place =
                            self.place_with_projection(init_place.clone(), field_proj);
                        let temp_local = self.alloc_local(field_pat.ty, Mutability::Not, span);
                        self.push_stmt(glyim_mir::StatementKind::StorageLive(temp_local), span);
                        self.push_stmt(
                            glyim_mir::StatementKind::Assign(
                                glyim_mir::Place::new(temp_local),
                                glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(field_place)),
                            ),
                            span,
                        );
                        self.bind_pattern(field_pat, Some(temp_local), span);
                    }
                }
            }
            thir::PatternKind::Struct {
                adt_id,
                variant_idx,
                fields,
                rest: _rest,
            } => {
                if let Some(init) = init_local {
                    let init_place = glyim_mir::Place::new(init);
                    for field_pat in fields {
                        let field_idx =
                            self.ctx
                                .field_index_by_name(*adt_id, *variant_idx, field_pat.field);
                        let field_idx = match field_idx {
                            Some(idx) => idx,
                            None => continue,
                        };
                        let field_proj = ProjectionElem::Field(field_idx);
                        let field_place =
                            self.place_with_projection(init_place.clone(), field_proj);
                        let temp_local =
                            self.alloc_local(field_pat.pattern.ty, Mutability::Not, field_pat.span);
                        self.push_stmt(
                            glyim_mir::StatementKind::StorageLive(temp_local),
                            field_pat.span,
                        );
                        self.push_stmt(
                            glyim_mir::StatementKind::Assign(
                                glyim_mir::Place::new(temp_local),
                                glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(field_place)),
                            ),
                            field_pat.span,
                        );
                        self.bind_pattern(&field_pat.pattern, Some(temp_local), field_pat.span);
                    }
                }
            }
            thir::PatternKind::Or(pats) => {
                if let Some(first_pat) = pats.first() {
                    self.bind_pattern(first_pat, init_local, span);
                }
            }
            thir::PatternKind::Literal(_) => {}
            thir::PatternKind::ConstBlock(_) => {
                tracing::warn!("STUB: const block pattern binding not implemented");
            }
            thir::PatternKind::Error => {}
        }
    }

    // ---- Literal lowering ----

    fn lower_literal(
        &self,
        lit: &thir::Literal,
        ty: Ty,
        span: glyim_span::Span,
    ) -> glyim_mir::MirConst {
        match lit {
            thir::Literal::Int(val, _) => glyim_mir::MirConst {
                kind: glyim_mir::MirConstKind::Int(*val),
                ty,
                span,
            },
            thir::Literal::Uint(val, _) => glyim_mir::MirConst {
                kind: glyim_mir::MirConstKind::Uint(*val),
                ty,
                span,
            },
            thir::Literal::FloatBits(val, _fty) => glyim_mir::MirConst {
                kind: glyim_mir::MirConstKind::FloatBits(*val),
                ty,
                span,
            },
            thir::Literal::Bool(val) => glyim_mir::MirConst {
                kind: glyim_mir::MirConstKind::Bool(*val),
                ty,
                span,
            },
            thir::Literal::Char(ch) => glyim_mir::MirConst {
                kind: glyim_mir::MirConstKind::Int(*ch as i128),
                ty,
                span,
            },
            thir::Literal::String(name) => glyim_mir::MirConst {
                kind: glyim_mir::MirConstKind::String(*name),
                ty,
                span,
            },
            thir::Literal::Unit => glyim_mir::MirConst {
                kind: glyim_mir::MirConstKind::Unit,
                ty,
                span,
            },
        }
    }

    // ---- Match lowering ----

    fn lower_match(
        &mut self,
        scrutinee: &thir::Expr,
        arms: &[thir::MatchArm],
        result_ty: Ty,
        span: glyim_span::Span,
    ) -> glyim_mir::Rvalue {
        let discr_op = self.lower_expr_to_operand(scrutinee);
        let merge_bb = self.new_block();
        let dest_local = self.alloc_local(result_ty, Mutability::Mut, span);
        let dest_place = glyim_mir::Place::new(dest_local);

        let mut arm_blocks: Vec<(BasicBlockIdx, &thir::MatchArm)> = Vec::new();
        let mut targets = Vec::new();
        for (i, arm) in arms.iter().enumerate() {
            let arm_bb = self.new_block();
            if i < arms.len() - 1 {
                let val = self.pattern_to_switch_value(&arm.pat);
                targets.push((val, arm_bb));
            }
            arm_blocks.push((arm_bb, arm));
        }

        let otherwise_bb = arm_blocks.last().map(|(bb, _)| *bb).unwrap_or(merge_bb);
        let switch_targets =
            glyim_mir::SwitchTargets::new(targets.into_boxed_slice(), otherwise_bb);

        self.terminate(
            glyim_mir::TerminatorKind::SwitchInt {
                discr: discr_op,
                switch_ty: scrutinee.ty,
                targets: switch_targets,
            },
            span,
        );

        for (arm_bb, arm) in arm_blocks.iter() {
            self.current_block = Some(*arm_bb);

            if let Some(guard) = &arm.guard {
                let guard_op = self.lower_expr_to_operand(guard);
                let arm_body_bb = self.new_block();
                let arm_skip_bb = self.new_block();

                let guard_targets =
                    glyim_mir::SwitchTargets::new(Box::new([(1, arm_body_bb)]), arm_skip_bb);
                self.terminate(
                    glyim_mir::TerminatorKind::SwitchInt {
                        discr: guard_op,
                        switch_ty: guard.ty,
                        targets: guard_targets,
                    },
                    guard.span,
                );

                self.current_block = Some(arm_body_bb);
                let arm_val = self.lower_expr_to_rvalue(&arm.body);
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(dest_place.clone(), arm_val),
                    arm.body.span,
                );
                self.terminate(
                    glyim_mir::TerminatorKind::Goto { target: merge_bb },
                    arm.body.span,
                );

                self.current_block = Some(arm_skip_bb);
                self.terminate(
                    glyim_mir::TerminatorKind::Goto {
                        target: otherwise_bb,
                    },
                    guard.span,
                );
            } else {
                let arm_val = self.lower_expr_to_rvalue(&arm.body);
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(dest_place.clone(), arm_val),
                    arm.body.span,
                );
                self.terminate(
                    glyim_mir::TerminatorKind::Goto { target: merge_bb },
                    arm.body.span,
                );
            }
        }

        self.current_block = Some(merge_bb);
        glyim_mir::Rvalue::Use(glyim_mir::Operand::Move(dest_place))
    }

    fn pattern_to_switch_value(&self, pat: &thir::Pattern) -> u128 {
        match &pat.kind {
            thir::PatternKind::Literal(lit) => match lit {
                thir::Literal::Int(v, _) => *v as u128,
                thir::Literal::Uint(v, _) => *v,
                thir::Literal::Bool(b) => *b as u128,
                thir::Literal::Char(ch) => *ch as u128,
                _ => u128::MAX,
            },
            _ => u128::MAX,
        }
    }

    // ---- Field resolution helpers ----

    fn resolve_field_index(
        &self,
        receiver_ty: Ty,
        field_name: glyim_core::interner::Name,
        _span: glyim_span::Span,
    ) -> Option<FieldIdx> {
        match self.ctx.ty_ctx().ty_kind(receiver_ty) {
            TyKind::Adt(adt_id, _substs) => {
                if let Some(idx) = self.ctx.field_index_by_name(*adt_id, 0, field_name) {
                    return Some(idx);
                }
                None
            }
            TyKind::Tuple(_substs) => {
                let name_str = self.ctx.ty_ctx().name_str(field_name);
                if let Ok(idx) = name_str.parse::<u32>() {
                    Some(FieldIdx::from_raw(idx))
                } else {
                    None
                }
            }
            thir::PatternKind::Range { start, end, inclusive } => {
                tracing::warn!("range pattern lowering not implemented");
                let err_place = self.alloc_local(Ty::ERROR, Mutability::Mut, span);
                self.push_stmt(glyim_mir::StatementKind::StorageLive(err_place), span);
                (glyim_mir::Place::new(err_place), None)
            }
            _ => None,
        }
    }

    // ---- Place helpers ----

    fn place_with_projection(
        &self,
        base: glyim_mir::Place,
        elem: ProjectionElem,
    ) -> glyim_mir::Place {
        let mut proj = base.projection.to_vec();
        proj.push(elem);
        glyim_mir::Place {
            local: base.local,
            projection: proj.into_boxed_slice(),
        }
    }
}
