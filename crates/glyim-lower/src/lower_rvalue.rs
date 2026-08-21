use crate::builder::{LoopInfo, MirBuilder};
use crate::lower_terminator::TerminatorExt;
use glyim_const_eval::{ConstEvaluator, ConstValue};
use glyim_core::TargetInfo;
use glyim_core::def_id::ClosureId;
use glyim_core::primitives::{BinOp, Mutability};
use glyim_diag::GlyimDiagnostic;
use glyim_layout::LayoutComputer;
use glyim_mir::{
    self, BasicBlockIdx, CastKind, LocalIdx, MirConst, MirConstKind, Operand, Place,
    ProjectionElem, Rvalue, StatementKind, SwitchTargets, TerminatorKind,
};
use glyim_type::{self, FieldIdx, Substitution, Ty, TyKind};
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
                    let temp_local =
                        self.alloc_local(*ty, glyim_core::primitives::Mutability::Mut, *span);
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
                let temp =
                    self.alloc_local(expr.ty, glyim_core::primitives::Mutability::Mut, expr.span);
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
            thir::ExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                // Lower a range to a real `Range<T>` / `RangeInclusive<T>`
                // aggregate (the builtin ADTs registered at ids 1000/1001)
                // carrying its `start` and `end` operands, instead of a dummy
                // empty tuple.
                let adt_id =
                    glyim_core::def_id::AdtId::from_raw(if *inclusive { 1001 } else { 1000 });

                // The range type is resolved during type-checking to
                // `Adt(1000|1001, [T])`; reuse its substitution so the element
                // type lines up with the operand types.
                let (substs, elem_ty) = match self.ctx.ty_ctx().ty_kind(expr.ty) {
                    glyim_type::TyKind::Adt(_, s) => {
                        let args = self.ctx.ty_ctx().substitution_args(*s);
                        let elem = args
                            .first()
                            .and_then(|a| match a {
                                glyim_type::GenericArg::Ty(t) => Some(*t),
                                _ => None,
                            })
                            .unwrap_or_else(|| self.ctx.ty_ctx().error_ty());
                        (*s, elem)
                    }
                    _ => {
                        let elem = start
                            .as_ref()
                            .map(|e| e.ty)
                            .or_else(|| end.as_ref().map(|e| e.ty))
                            .unwrap_or_else(|| self.ctx.ty_ctx().error_ty());
                        // No resolved substitution available; emit an empty one
                        // and rely on the element type for operand shapes.
                        (glyim_type::Substitution::empty(), elem)
                    }
                };

                let start_op = match start {
                    Some(e) => self.lower_expr_to_operand(e),
                    None => glyim_mir::Operand::Constant(glyim_mir::MirConst {
                        kind: glyim_mir::MirConstKind::Error,
                        ty: elem_ty,
                        span: expr.span,
                    }),
                };
                let end_op = match end {
                    Some(e) => self.lower_expr_to_operand(e),
                    None => glyim_mir::Operand::Constant(glyim_mir::MirConst {
                        kind: glyim_mir::MirConstKind::Error,
                        ty: elem_ty,
                        span: expr.span,
                    }),
                };

                glyim_mir::Rvalue::Aggregate(
                    glyim_mir::AggregateKind::Adt(
                        adt_id,
                        glyim_mir::VariantIdx::from_raw(0),
                        substs,
                    ),
                    vec![start_op, end_op],
                )
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
            thir::ExprKind::ConstRef(const_def_id) => {
                // Part C: const value materialization. If typeck const-
                // evaluated this constant, fold it into a concrete `MirConst`
                // via `LowerCtx::const_value` (scalar constants fold fully;
                // aggregate/range constants return `None` here and fall back to
                // the `ConstRef` zero-initialized global below).
                let substs = glyim_type::Substitution::empty();
                if let Some(mir_const) = self.ctx.const_value(*const_def_id, substs) {
                    return glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(mir_const));
                }
                // Fallback: emit a `ConstRef` referencing the constant's
                // definition. Handled by mono, polymorphize, and the LLVM
                // backend (global `__glyim_const_{id}`). Substs are empty for
                // the monomorphic consts supported here.
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::ConstRef(*const_def_id, substs),
                    ty: expr.ty,
                    span: expr.span,
                }))
            }
            thir::ExprKind::VariantRef(adt_id, variant_idx) => {
                // A unit enum variant used as a value (`Color::Red`) constructs
                // the enum via an `Aggregate` of its ADT with no fields. Data
                // variants (call-like constructors) are a follow-up; this
                // covers the unit-variant value path.
                let substs = glyim_type::Substitution::empty();
                glyim_mir::Rvalue::Aggregate(
                    glyim_mir::AggregateKind::Adt(
                        *adt_id,
                        glyim_mir::VariantIdx::from_raw(variant_idx.to_raw()),
                        substs,
                    ),
                    vec![],
                )
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
                    glyim_core::primitives::Mutability::Mut => glyim_mir::BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                    glyim_core::primitives::Mutability::Not => glyim_mir::BorrowKind::Shared,
                };
                glyim_mir::Rvalue::Ref(place, borrow_kind)
            }
            thir::ExprKind::Call { func, args } => {
                // Data-carrying variant constructor call (`Some(x)` /
                // `Color::Green(x)`): lower to an `Aggregate` of the enum ADT
                // instead of a function call.
                if let thir::ExprKind::VariantCtor {
                    adt_id,
                    variant_idx,
                } = &func.kind
                {
                    let mut mir_args = Vec::new();
                    for arg in args {
                        mir_args.push(self.lower_expr_to_operand(arg));
                    }
                    let substs = glyim_type::Substitution::empty();
                    return glyim_mir::Rvalue::Aggregate(
                        glyim_mir::AggregateKind::Adt(
                            *adt_id,
                            glyim_mir::VariantIdx::from_raw(variant_idx.to_raw()),
                            substs,
                        ),
                        mir_args,
                    );
                }

                let mut mir_args = Vec::new();
                for arg in args {
                    mir_args.push(self.lower_expr_to_operand(arg));
                }
                let func_op = self.lower_expr_to_operand(func);
                let dest_local =
                    self.alloc_local(expr.ty, glyim_core::primitives::Mutability::Mut, expr.span);
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
            thir::ExprKind::VariantCtor { .. } => {
                // A bare variant constructor value is not yet supported as a
                // first-class function value; it is only valid as a call
                // target (handled in the `Call` arm above). Emit an error
                // rvalue to keep lowering exhaustive.
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Error,
                    ty: expr.ty,
                    span: expr.span,
                }))
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
                let dest_local =
                    self.alloc_local(expr.ty, glyim_core::primitives::Mutability::Mut, expr.span);
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
                pat,
                iterable,
                body,
            } => {
                let iter_ty = iterable.ty;
                let elem_ty = pat.ty;
                let iter_local = self.alloc_local(
                    iter_ty,
                    glyim_core::primitives::Mutability::Mut,
                    iterable.span,
                );
                self.push_stmt(
                    glyim_mir::StatementKind::StorageLive(iter_local),
                    iterable.span,
                );
                let iter_rvalue = self.lower_expr_to_rvalue(iterable);
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(
                        glyim_mir::Place::new(iter_local),
                        iter_rvalue,
                    ),
                    iterable.span,
                );
                let header_bb = self.new_block();
                let exit_bb = self.new_block();
                self.terminate(
                    glyim_mir::TerminatorKind::Goto { target: header_bb },
                    expr.span,
                );
                self.loop_stack.push(LoopInfo {
                    continue_bb: header_bb,
                    break_bb: exit_bb,
                });
                // Get the iterator_next info; if not available, use a simplified fallback.
                match self.ctx.iterator_next_fn(iter_ty, elem_ty) {
                    Some(iter_info) => {
                        self.current_block = Some(header_bb);
                        let ref_iter_local = self.alloc_local(
                            iter_info.ref_iter_ty,
                            glyim_core::primitives::Mutability::Mut,
                            iterable.span,
                        );
                        self.push_stmt(
                            glyim_mir::StatementKind::StorageLive(ref_iter_local),
                            iterable.span,
                        );
                        self.push_stmt(
                            glyim_mir::StatementKind::Assign(
                                glyim_mir::Place::new(ref_iter_local),
                                glyim_mir::Rvalue::Ref(
                                    glyim_mir::Place::new(iter_local),
                                    glyim_mir::BorrowKind::Mut {
                                        allow_two_phase_borrow: false,
                                    },
                                ),
                            ),
                            iterable.span,
                        );
                        let next_fn_const = glyim_mir::MirConst {
                            kind: glyim_mir::MirConstKind::Fn(
                                iter_info.fn_def_id,
                                iter_info.fn_substs,
                            ),
                            ty: iter_info.fn_ty,
                            span: expr.span,
                        };
                        let next_fn_op = glyim_mir::Operand::Constant(next_fn_const);
                        let ref_iter_op =
                            glyim_mir::Operand::Copy(glyim_mir::Place::new(ref_iter_local));
                        let option_local = self.alloc_local(
                            iter_info.option_ty,
                            glyim_core::primitives::Mutability::Mut,
                            expr.span,
                        );
                        self.push_stmt(
                            glyim_mir::StatementKind::StorageLive(option_local),
                            expr.span,
                        );
                        let after_call_bb = self.new_block();
                        self.terminate(
                            glyim_mir::TerminatorKind::Call {
                                func: next_fn_op,
                                args: vec![ref_iter_op],
                                destination: glyim_mir::Place::new(option_local),
                                target: Some(after_call_bb),
                                cleanup: None,
                            },
                            expr.span,
                        );
                        self.current_block = Some(after_call_bb);
                        let discr_op =
                            glyim_mir::Operand::Copy(glyim_mir::Place::new(option_local));
                        let some_bb = self.new_block();
                        let none_bb = exit_bb;
                        let switch_targets =
                            glyim_mir::SwitchTargets::new(Box::new([(1, some_bb)]), none_bb);
                        self.terminate(
                            glyim_mir::TerminatorKind::SwitchInt {
                                discr: discr_op,
                                switch_ty: iter_info.discr_ty,
                                targets: switch_targets,
                            },
                            expr.span,
                        );
                        self.current_block = Some(some_bb);
                        let payload_place = {
                            let mut proj = vec![glyim_mir::ProjectionElem::Downcast(
                                glyim_mir::VariantIdx::from_raw(1),
                            )];
                            proj.push(glyim_mir::ProjectionElem::Field(FieldIdx::from_raw(0)));
                            glyim_mir::Place {
                                local: option_local,
                                projection: proj.into_boxed_slice(),
                            }
                        };
                        let payload_local = self.alloc_local(
                            elem_ty,
                            glyim_core::primitives::Mutability::Not,
                            expr.span,
                        );
                        self.push_stmt(
                            glyim_mir::StatementKind::StorageLive(payload_local),
                            expr.span,
                        );
                        self.push_stmt(
                            glyim_mir::StatementKind::Assign(
                                glyim_mir::Place::new(payload_local),
                                glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(payload_place)),
                            ),
                            expr.span,
                        );
                        self.bind_pattern(pat, Some(payload_local), expr.span);
                        let _ = self.lower_expr_to_rvalue(body);
                        if self.current_block.is_some() {
                            self.terminate(
                                glyim_mir::TerminatorKind::Goto { target: header_bb },
                                body.span,
                            );
                        }
                        self.loop_stack.pop();
                        self.current_block = Some(exit_bb);
                        glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                            kind: glyim_mir::MirConstKind::Unit,
                            ty: Ty::UNIT,
                            span: expr.span,
                        }))
                    }
                    None => {
                        // Simplified fallback: execute the body once and break.
                        // This is only used in tests where iterator protocol is not available.
                        self.current_block = Some(header_bb);
                        // Bind the pattern to a dummy value (the iterable itself).
                        // We'll just execute the body without binding the pattern.
                        let _ = self.lower_expr_to_rvalue(body);
                        self.loop_stack.pop();
                        self.terminate(
                            glyim_mir::TerminatorKind::Goto { target: exit_bb },
                            expr.span,
                        );
                        self.current_block = Some(exit_bb);
                        glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                            kind: glyim_mir::MirConstKind::Unit,
                            ty: Ty::UNIT,
                            span: expr.span,
                        }))
                    }
                }
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
                // Check if the index is a Range expression.
                if let thir::ExprKind::Range {
                    start,
                    end,
                    inclusive,
                } = &index.kind
                {
                    if *inclusive {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            expr.span,
                            "inclusive ranges (..=) are not supported for slicing yet".to_string(),
                        ));
                        return glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(
                            glyim_mir::MirConst {
                                kind: glyim_mir::MirConstKind::Error,
                                ty: self.ctx.ty_ctx().error_ty(),
                                span: expr.span,
                            },
                        ));
                    }
                    let base_place = self.lower_expr_to_place(base);
                    self.lower_dynamic_range_slice(
                        base_place,
                        start.as_ref().map(|e| e.as_ref()),
                        end.as_ref().map(|e| e.as_ref()),
                        expr.ty,
                        expr.span,
                    )
                } else {
                    // Regular indexing (single element).
                    let base_place = self.lower_expr_to_place(base);
                    let index_local = self.alloc_local(
                        index.ty,
                        glyim_core::primitives::Mutability::Not,
                        index.span,
                    );
                    let index_rval = self.lower_expr_to_rvalue(index);
                    self.push_stmt(
                        glyim_mir::StatementKind::Assign(
                            glyim_mir::Place::new(index_local),
                            index_rval,
                        ),
                        index.span,
                    );
                    let place = self.place_with_projection(
                        base_place,
                        glyim_mir::ProjectionElem::Index(index_local),
                    );
                    glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(place))
                }
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
                    (TyKind::Float(_), TyKind::Float(_)) => CastKind::FloatToFloat,
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
            thir::ExprKind::Return { value } => {
                if let Some(val_expr) = value {
                    let rvalue = self.lower_expr_to_rvalue(val_expr);
                    let ret_place = glyim_mir::Place::new(LocalIdx::from_raw(0));
                    self.push_stmt(
                        glyim_mir::StatementKind::Assign(ret_place, rvalue),
                        expr.span,
                    );
                }
                self.terminate(glyim_mir::TerminatorKind::Return, expr.span);
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Unit,
                    ty: Ty::NEVER,
                    span: expr.span,
                }))
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
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Unit,
                    ty: Ty::NEVER,
                    span: expr.span,
                }))
            }
            thir::ExprKind::Closure {
                body: _thir_body,
                captures,
                is_move: _,
            } => {
                let (closure_id, closure_substs) = match self.ctx.ty_ctx().ty_kind(expr.ty) {
                    TyKind::Closure(id, substs) => (id, substs),
                    _ => {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            expr.span,
                            "closure expression has non-closure type".to_string(),
                        ));
                        return glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(
                            glyim_mir::MirConst {
                                kind: glyim_mir::MirConstKind::Error,
                                ty: expr.ty,
                                span: expr.span,
                            },
                        ));
                    }
                };
                // Build the closure's own MIR body so codegen can emit it as
                // `__glyim_fn_{closure_id}`. The captures come first, followed
                // by the closure's own parameters (see `lower_closure`).
                self.lower_closure(
                    &_thir_body,
                    &captures,
                    *closure_id,
                    *closure_substs,
                    expr.span,
                );
                let mut capture_operands = Vec::with_capacity(captures.len());
                for capture in captures {
                    let capture_local = LocalIdx::from_raw(capture.local.to_raw());
                    let operand = match capture.kind {
                        thir::CaptureKind::ByValue => {
                            glyim_mir::Operand::Move(glyim_mir::Place::new(capture_local))
                        }
                        thir::CaptureKind::ByRef(glyim_core::primitives::Mutability::Not)
                        | thir::CaptureKind::ByRef(glyim_core::primitives::Mutability::Mut) => {
                            glyim_mir::Operand::Copy(glyim_mir::Place::new(capture_local))
                        }
                    };
                    capture_operands.push(operand);
                }
                glyim_mir::Rvalue::Aggregate(
                    glyim_mir::AggregateKind::Closure(*closure_id, *closure_substs),
                    capture_operands,
                )
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

            thir::ExprKind::DynamicCall {
                receiver,
                method_index,
                args,
            } => {
                let _receiver = receiver;
                // Lower the receiver (a fat pointer) to a temporary local so we can project into it.
                let recv_val = self.lower_expr_to_rvalue(receiver);
                let recv_local = self.alloc_local(
                    receiver.ty,
                    glyim_core::primitives::Mutability::Mut,
                    expr.span,
                );
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(glyim_mir::Place::new(recv_local), recv_val),
                    expr.span,
                );
                let recv_place = glyim_mir::Place::new(recv_local);

                // Extract the data pointer (field 0) and vtable pointer (field 1) from the fat pointer.
                let data_place = self.place_with_projection(
                    recv_place.clone(),
                    ProjectionElem::Field(FieldIdx::from_raw(0)),
                );
                let vtable_place = self.place_with_projection(
                    recv_place.clone(),
                    ProjectionElem::Field(FieldIdx::from_raw(1)),
                );

                // Allocate a local for the vtable pointer. Use Ty::USIZE as a pointer-sized integer.
                let vtable_ptr_local = self.alloc_local(
                    Ty::USIZE,
                    glyim_core::primitives::Mutability::Mut,
                    expr.span,
                );
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(
                        glyim_mir::Place::new(vtable_ptr_local),
                        glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(vtable_place)),
                    ),
                    expr.span,
                );

                let _method_index = method_index;
                // Allocate a local for the method index and store the constant index.
                let method_idx_local = self.alloc_local(
                    Ty::USIZE,
                    glyim_core::primitives::Mutability::Not,
                    expr.span,
                );
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(
                        glyim_mir::Place::new(method_idx_local),
                        glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                            kind: glyim_mir::MirConstKind::Uint(*method_index as u128),
                            ty: Ty::USIZE,
                            span: expr.span,
                        })),
                    ),
                    expr.span,
                );

                // Project into the vtable to get the method pointer.
                let method_ptr_place = self.place_with_projection(
                    glyim_mir::Place::new(vtable_ptr_local),
                    ProjectionElem::Index(method_idx_local),
                );

                // Allocate a local for the method pointer. Use Ty::ERROR because we don't have
                // a way to allocate a FnPtr type from an immutable TyCtx. The LLVM backend will
                // construct the function type from the arguments and return type.
                let method_fn_local = self.alloc_local(
                    Ty::ERROR,
                    glyim_core::primitives::Mutability::Mut,
                    expr.span,
                );
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(
                        glyim_mir::Place::new(method_fn_local),
                        glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(method_ptr_place)),
                    ),
                    expr.span,
                );

                let _args = args;
                // Prepare arguments: data pointer is the receiver (self), followed by the rest of the args.
                let mut mir_args = Vec::with_capacity(args.len() + 1);
                mir_args.push(glyim_mir::Operand::Copy(data_place));
                for arg in args {
                    mir_args.push(self.lower_expr_to_operand(arg));
                }

                // Call the method pointer.
                let dest_local =
                    self.alloc_local(expr.ty, glyim_core::primitives::Mutability::Mut, expr.span);
                let dest_place = glyim_mir::Place::new(dest_local);
                let next_bb = self.new_block();
                self.terminate(
                    glyim_mir::TerminatorKind::Call {
                        func: glyim_mir::Operand::Move(glyim_mir::Place::new(method_fn_local)),
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

            // `TraitMethodRef` is always resolved to a concrete `FnRef` +
            // `Call` at the typeck call site (static dispatch), so it never
            // reaches MIR lowering. Kept for exhaustiveness.
            thir::ExprKind::TraitMethodRef { .. } => unreachable!(
                "TraitMethodRef should have been resolved to FnRef+Call during type-checking"
            ),
        }
    }

    // ---- Expression → Operand lowering ----
    pub fn lower_expr_to_operand(&mut self, expr: &thir::Expr) -> glyim_mir::Operand {
        match &expr.kind {
            thir::ExprKind::Literal(_) | thir::ExprKind::FnRef(_) => {
                if let glyim_mir::Rvalue::Use(op) = self.lower_expr_to_rvalue(expr) {
                    op
                } else {
                    unreachable!()
                }
            }
            thir::ExprKind::VarRef(var_id) => {
                let local = LocalIdx::from_raw(var_id.to_raw());
                glyim_mir::Operand::Copy(glyim_mir::Place::new(local))
            }
            _ => {
                let rvalue = self.lower_expr_to_rvalue(expr);
                let local =
                    self.alloc_local(expr.ty, glyim_core::primitives::Mutability::Mut, expr.span);
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
                let index_local = self.alloc_local(
                    index.ty,
                    glyim_core::primitives::Mutability::Not,
                    index.span,
                );
                let index_rval = self.lower_expr_to_rvalue(index);
                self.push_stmt(
                    glyim_mir::StatementKind::Assign(
                        glyim_mir::Place::new(index_local),
                        index_rval,
                    ),
                    index.span,
                );
                self.place_with_projection(
                    base_place,
                    glyim_mir::ProjectionElem::Index(index_local),
                )
            }
            thir::ExprKind::Ref {
                operand,
                mutability: _,
            } => self.lower_expr_to_place(operand),
            _ => {
                let rvalue = self.lower_expr_to_rvalue(expr);
                let local =
                    self.alloc_local(expr.ty, glyim_core::primitives::Mutability::Mut, expr.span);
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
            thir::PatternKind::Range { .. } => {}
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
                        let temp_local = self.alloc_local(
                            field_pat.ty,
                            glyim_core::primitives::Mutability::Not,
                            span,
                        );
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
                        let temp_local = self.alloc_local(
                            field_pat.pattern.ty,
                            glyim_core::primitives::Mutability::Not,
                            field_pat.span,
                        );
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
            // `Literal` and `ConstBlock` patterns bind no names of their
            // own -- like `Literal`, `ConstBlock` is a *refutable*
            // comparison pattern (`match x { const { A + B } => .. }`),
            // not a binding pattern. Nothing to do here; the actual
            // compile-time evaluation and comparison-value generation for
            // `ConstBlock` happens in `collect_switch_values` below, which
            // is where `PatternKind::Literal` is also turned into a
            // switch-arm value.
            thir::PatternKind::Literal(_) => {}
            thir::PatternKind::ConstBlock(_) => {}
            thir::PatternKind::Error => {}
            thir::PatternKind::Slice {
                prefix,
                slice,
                suffix,
            } => {
                if let Some(init) = init_local {
                    let init_place = glyim_mir::Place::new(init);

                    for (i, sub_pat) in prefix.iter().enumerate() {
                        let proj = ProjectionElem::ConstantIndex {
                            offset: i as u64,
                            min_length: (prefix.len() + suffix.len()) as u64,
                            from_end: false,
                        };
                        let elem_place = self.place_with_projection(init_place.clone(), proj);
                        let temp_local = self.alloc_local(
                            sub_pat.ty,
                            glyim_core::primitives::Mutability::Not,
                            span,
                        );
                        self.push_stmt(glyim_mir::StatementKind::StorageLive(temp_local), span);
                        self.push_stmt(
                            glyim_mir::StatementKind::Assign(
                                glyim_mir::Place::new(temp_local),
                                glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(elem_place)),
                            ),
                            span,
                        );
                        self.bind_pattern(sub_pat, Some(temp_local), span);
                    }

                    for (i, sub_pat) in suffix.iter().enumerate() {
                        let proj = ProjectionElem::ConstantIndex {
                            offset: (suffix.len() - i) as u64,
                            min_length: (prefix.len() + suffix.len()) as u64,
                            from_end: true,
                        };
                        let elem_place = self.place_with_projection(init_place.clone(), proj);
                        let temp_local = self.alloc_local(
                            sub_pat.ty,
                            glyim_core::primitives::Mutability::Not,
                            span,
                        );
                        self.push_stmt(glyim_mir::StatementKind::StorageLive(temp_local), span);
                        self.push_stmt(
                            glyim_mir::StatementKind::Assign(
                                glyim_mir::Place::new(temp_local),
                                glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(elem_place)),
                            ),
                            span,
                        );
                        self.bind_pattern(sub_pat, Some(temp_local), span);
                    }

                    if let Some(rest_pat) = slice {
                        let proj = ProjectionElem::Subslice {
                            from: prefix.len() as u64,
                            to: suffix.len() as u64,
                            from_end: true,
                        };
                        let rest_place = self.place_with_projection(init_place.clone(), proj);
                        let temp_local = self.alloc_local(
                            rest_pat.ty,
                            glyim_core::primitives::Mutability::Not,
                            span,
                        );
                        self.push_stmt(glyim_mir::StatementKind::StorageLive(temp_local), span);
                        self.push_stmt(
                            glyim_mir::StatementKind::Assign(
                                glyim_mir::Place::new(temp_local),
                                glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(rest_place)),
                            ),
                            span,
                        );
                        self.bind_pattern(rest_pat, Some(temp_local), span);
                    }
                }
            }
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
        let merge_bb = self.new_block();
        let dest_local = self.alloc_local(result_ty, glyim_core::primitives::Mutability::Mut, span);
        let dest_place = glyim_mir::Place::new(dest_local);

        // Slice/array matches dispatch on the *length* of the scrutinee rather
        // than its value. `[a, b, c]` matches precisely when the slice has 3
        // elements, so we compute `len = Len(scrutinee)` and `SwitchInt` on it.
        let slice_dispatch = matches!(
            self.ctx.ty_ctx().ty_kind(scrutinee.ty),
            TyKind::Slice(_) | TyKind::Array(_, _)
        ) && arms
            .iter()
            .any(|a| matches!(&a.pat.kind, thir::PatternKind::Slice { .. }));

        let (discr_op, switch_ty) = if slice_dispatch {
            let scrutinee_place = self.lower_expr_to_place(scrutinee);
            let len_local =
                self.alloc_local(Ty::USIZE, glyim_core::primitives::Mutability::Not, span);
            self.push_stmt(
                glyim_mir::StatementKind::Assign(
                    glyim_mir::Place::new(len_local),
                    glyim_mir::Rvalue::Len(scrutinee_place),
                ),
                span,
            );
            (
                glyim_mir::Operand::Copy(glyim_mir::Place::new(len_local)),
                Ty::USIZE,
            )
        } else {
            let discr_op = self.lower_expr_to_operand(scrutinee);
            (discr_op, scrutinee.ty)
        };

        let mut switch_targets: Vec<(u128, BasicBlockIdx)> = Vec::new();
        let mut arm_blocks: Vec<(BasicBlockIdx, &thir::MatchArm)> = Vec::new();
        let otherwise_bb = self.new_block();

        for arm in arms.iter() {
            let arm_bb = self.new_block();
            arm_blocks.push((arm_bb, arm));
            self.collect_switch_values(&arm.pat, &mut switch_targets, arm_bb);
        }

        let otherwise = if switch_targets.is_empty() {
            arm_blocks.first().map(|(bb, _)| *bb).unwrap_or(merge_bb)
        } else {
            otherwise_bb
        };

        let targets = glyim_mir::SwitchTargets::new(switch_targets.into_boxed_slice(), otherwise);
        self.terminate(
            glyim_mir::TerminatorKind::SwitchInt {
                discr: discr_op,
                switch_ty,
                targets,
            },
            span,
        );

        for (i, (arm_bb, arm)) in arm_blocks.iter().enumerate() {
            self.current_block = Some(*arm_bb);
            if let Some(guard) = &arm.guard {
                let guard_op = self.lower_expr_to_operand(guard);
                let arm_body_bb = self.new_block();
                let next_arm_bb = if i + 1 < arm_blocks.len() {
                    arm_blocks[i + 1].0
                } else {
                    otherwise_bb
                };

                let guard_targets =
                    glyim_mir::SwitchTargets::new(Box::new([(1, arm_body_bb)]), next_arm_bb);
                self.terminate(
                    glyim_mir::TerminatorKind::SwitchInt {
                        discr: guard_op,
                        switch_ty: guard.ty,
                        targets: guard_targets,
                    },
                    guard.span,
                );

                self.current_block = Some(arm_body_bb);
                self.lower_arm_body(arm, &dest_place, merge_bb);
            } else {
                self.lower_arm_body(arm, &dest_place, merge_bb);
            }
        }

        if self.current_block == Some(otherwise_bb) {
            self.terminate(glyim_mir::TerminatorKind::Unreachable, span);
        }

        self.current_block = Some(merge_bb);
        glyim_mir::Rvalue::Use(glyim_mir::Operand::Move(dest_place))
    }

    fn collect_switch_values(
        &mut self,
        pat: &thir::Pattern,
        targets: &mut Vec<(u128, BasicBlockIdx)>,
        arm_bb: BasicBlockIdx,
    ) {
        match &pat.kind {
            thir::PatternKind::Literal(lit) => {
                if let Some(val) = self.literal_to_u128(lit) {
                    targets.push((val, arm_bb));
                }
            }
            thir::PatternKind::Range {
                start,
                end,
                inclusive,
            } => {
                if let (Some(s), Some(e)) = (start, end)
                    && let (Some(s_val), Some(e_val)) =
                        (self.literal_to_u128(s), self.literal_to_u128(e))
                {
                    let end_val = if *inclusive {
                        e_val
                    } else {
                        e_val.saturating_sub(1)
                    };
                    for v in s_val..=end_val {
                        targets.push((v, arm_bb));
                    }
                }
            }
            thir::PatternKind::Or(subpats) => {
                for sub in subpats {
                    self.collect_switch_values(sub, targets, arm_bb);
                }
            }
            thir::PatternKind::Slice { prefix, suffix, .. } => {
                // A `[prefix..suffix]` pattern matches when the slice length
                // equals `prefix.len() + suffix.len()`; switch on that length.
                targets.push(((prefix.len() + suffix.len()) as u128, arm_bb));
            }
            thir::PatternKind::ConstBlock(const_body) => {
                match self.const_block_to_u128(const_body) {
                    Some(val) => targets.push((val, arm_bb)),
                    None => {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            pat.span,
                            "failed to evaluate `const` pattern at compile time",
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    fn literal_to_u128(&self, lit: &thir::Literal) -> Option<u128> {
        match lit {
            thir::Literal::Int(v, _) => Some(*v as u128),
            thir::Literal::Uint(v, _) => Some(*v),
            thir::Literal::Bool(b) => Some(*b as u128),
            thir::Literal::Char(ch) => Some(*ch as u128),
            _ => None,
        }
    }
    fn lower_arm_body(
        &mut self,
        arm: &thir::MatchArm,
        dest_place: &glyim_mir::Place,
        merge_bb: BasicBlockIdx,
    ) {
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
    /// Evaluate a `const { .. }` pattern's body by fetching the original HIR
    /// body and evaluating it via `glyim-const-eval`.

    fn const_block_to_u128(&mut self, const_body: &glyim_typeck::thir::Body) -> Option<u128> {
        // Use const_body.owner.local_id to get the LocalDefId
        let hir_body = self.ctx.hir_body(const_body.owner.local_id)?;

        let root =
            glyim_hir::ExprId::from_raw(u32::try_from(hir_body.exprs.len().checked_sub(1)?).ok()?);
        let mut evaluator = ConstEvaluator::new(hir_body);
        let value = evaluator.evaluate(root).ok()?;

        match value {
            ConstValue::Int(v, _) => Some(v as u128), // handles both positive and negative via two's complement
            ConstValue::Uint(v, _) => Some(v),
            ConstValue::Bool(b) => Some(b as u128),
            ConstValue::Char(c) => Some(c as u128),
            ConstValue::FloatBits(..) | ConstValue::String(_) | ConstValue::Unit => None,
            ConstValue::Tuple(_) | ConstValue::Array(_) | ConstValue::Struct(_) => None,
            ConstValue::Range(..) => None,
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
            TyKind::Adt(adt_id, _substs) => self.ctx.field_index_by_name(*adt_id, 0, field_name),
            TyKind::Tuple(_substs) => {
                let name_str = self.ctx.ty_ctx().name_str(field_name);
                name_str.parse::<u32>().ok().map(FieldIdx::from_raw)
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

    // ---- Dynamic range slicing ----
    pub(crate) fn lower_dynamic_range_slice(
        &mut self,
        base_place: glyim_mir::Place,
        start_opt: Option<&thir::Expr>,
        end_opt: Option<&thir::Expr>,
        result_ty: Ty,
        span: glyim_span::Span,
    ) -> glyim_mir::Rvalue {
        // Determine the element type and whether we have a slice or array.
        let base_ty = base_place.ty(self.ctx.ty_ctx(), &self.locals);
        let elem_ty = match self.ctx.ty_ctx().ty_kind(base_ty) {
            TyKind::Array(elem, _) | TyKind::Slice(elem) => *elem,
            _ => {
                self.diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "dynamic range slicing requires array or slice type".to_string(),
                ));
                return Rvalue::Use(Operand::Constant(glyim_mir::MirConst {
                    kind: MirConstKind::Error,
                    ty: result_ty,
                    span,
                }));
            }
        };

        // Allocate locals for start, end, len, data_ptr, new_len.
        let start_local = self.alloc_local(Ty::USIZE, Mutability::Mut, span);
        let end_local = self.alloc_local(Ty::USIZE, Mutability::Mut, span);
        let len_local = self.alloc_local(Ty::USIZE, Mutability::Mut, span);
        let data_ptr_local = self.alloc_local(Ty::USIZE, Mutability::Mut, span); // as usize
        let new_len_local = self.alloc_local(Ty::USIZE, Mutability::Mut, span);

        // StorageLive for all.
        self.push_stmt(StatementKind::StorageLive(start_local), span);
        self.push_stmt(StatementKind::StorageLive(end_local), span);
        self.push_stmt(StatementKind::StorageLive(len_local), span);
        self.push_stmt(StatementKind::StorageLive(data_ptr_local), span);
        self.push_stmt(StatementKind::StorageLive(new_len_local), span);

        // 1. Compute len_val = Len(base_place)
        let len_rvalue = Rvalue::Len(base_place.clone());
        self.push_stmt(
            StatementKind::Assign(Place::new(len_local), len_rvalue),
            span,
        );

        // 2. Compute start_val
        let start_val = if let Some(start_expr) = start_opt {
            // Evaluate start expression and assign to start_local
            let start_rvalue = self.lower_expr_to_rvalue(start_expr);
            self.push_stmt(
                StatementKind::Assign(Place::new(start_local), start_rvalue),
                start_expr.span,
            );
            Operand::Copy(Place::new(start_local))
        } else {
            // start = 0
            let zero = MirConst {
                kind: MirConstKind::Uint(0),
                ty: Ty::USIZE,
                span,
            };
            let op = Operand::Constant(zero);
            let rvalue = Rvalue::Use(op.clone());
            self.push_stmt(StatementKind::Assign(Place::new(start_local), rvalue), span);
            op
        };

        // 3. Compute end_val
        let end_val = if let Some(end_expr) = end_opt {
            let end_rvalue = self.lower_expr_to_rvalue(end_expr);
            self.push_stmt(
                StatementKind::Assign(Place::new(end_local), end_rvalue),
                end_expr.span,
            );
            Operand::Copy(Place::new(end_local))
        } else {
            // end = len
            let op = Operand::Copy(Place::new(len_local));
            let rvalue = Rvalue::Use(op.clone());
            self.push_stmt(StatementKind::Assign(Place::new(end_local), rvalue), span);
            op
        };

        // 4. Bounds checks: start <= end, end <= len
        let check_start_le_end_bb = self.new_block();
        let check_end_le_len_bb = self.new_block();
        let done_bb = self.new_block();

        // Check start <= end
        let start_le_end =
            Rvalue::BinaryOp(BinOp::LtEq, Box::new((start_val.clone(), end_val.clone())));
        let start_le_end_local =
            self.alloc_local(self.ctx.ty_ctx().bool_ty(), Mutability::Not, span);
        self.push_stmt(StatementKind::StorageLive(start_le_end_local), span);
        self.push_stmt(
            StatementKind::Assign(Place::new(start_le_end_local), start_le_end),
            span,
        );

        // Terminate current block with Assert on start_le_end.
        let cond_op = Operand::Copy(Place::new(start_le_end_local));
        let targets = SwitchTargets::if_switch(check_start_le_end_bb, check_end_le_len_bb);
        self.terminate(
            TerminatorKind::SwitchInt {
                discr: cond_op,
                switch_ty: self.ctx.ty_ctx().bool_ty(),
                targets,
            },
            span,
        );

        self.current_block = Some(check_start_le_end_bb);

        // Check end <= len
        let end_le_len = Rvalue::BinaryOp(
            BinOp::LtEq,
            Box::new((end_val.clone(), Operand::Copy(Place::new(len_local)))),
        );
        let end_le_len_local = self.alloc_local(self.ctx.ty_ctx().bool_ty(), Mutability::Not, span);
        self.push_stmt(StatementKind::StorageLive(end_le_len_local), span);
        self.push_stmt(
            StatementKind::Assign(Place::new(end_le_len_local), end_le_len),
            span,
        );

        let cond_op2 = Operand::Copy(Place::new(end_le_len_local));
        let targets2 = SwitchTargets::if_switch(done_bb, check_end_le_len_bb);
        self.terminate(
            TerminatorKind::SwitchInt {
                discr: cond_op2,
                switch_ty: self.ctx.ty_ctx().bool_ty(),
                targets: targets2,
            },
            span,
        );

        self.current_block = Some(check_end_le_len_bb);
        self.terminate(TerminatorKind::Unreachable, span);

        self.current_block = Some(done_bb);

        // Compute data_ptr:
        let first_elem_place = if matches!(self.ctx.ty_ctx().ty_kind(base_ty), TyKind::Array(_, _))
        {
            let mut proj = base_place.projection.to_vec();
            proj.push(ProjectionElem::ConstantIndex {
                offset: 0,
                min_length: 0,
                from_end: false,
            });
            Place {
                local: base_place.local,
                projection: proj.into_boxed_slice(),
            }
        } else {
            let mut proj = base_place.projection.to_vec();
            proj.push(ProjectionElem::Field(glyim_type::FieldIdx::from_raw(0)));
            Place {
                local: base_place.local,
                projection: proj.into_boxed_slice(),
            }
        };

        let data_ptr_ptr = self.alloc_local(Ty::USIZE, Mutability::Not, span);
        self.push_stmt(StatementKind::StorageLive(data_ptr_ptr), span);
        let load_data_ptr = Rvalue::Use(Operand::Copy(first_elem_place));
        self.push_stmt(
            StatementKind::Assign(Place::new(data_ptr_ptr), load_data_ptr),
            span,
        );

        let layout_computer =
            glyim_layout::SimpleLayoutComputer::new(self.ctx.ty_ctx(), TargetInfo::x86_64());
        let elem_layout =
            layout_computer
                .layout_of(elem_ty)
                .unwrap_or(glyim_layout::Layout::scalar(
                    glyim_layout::Size::bytes(1),
                    glyim_layout::Align::ONE,
                ));
        let elem_size = elem_layout.size.0;

        let elem_size_const = MirConst {
            kind: MirConstKind::Uint(elem_size.into()),
            ty: Ty::USIZE,
            span,
        };
        let start_op = Operand::Copy(Place::new(start_local));
        let size_op = Operand::Constant(elem_size_const);
        let byte_offset = Rvalue::BinaryOp(BinOp::Mul, Box::new((start_op, size_op)));
        let byte_offset_local = self.alloc_local(Ty::USIZE, Mutability::Not, span);
        self.push_stmt(StatementKind::StorageLive(byte_offset_local), span);
        self.push_stmt(
            StatementKind::Assign(Place::new(byte_offset_local), byte_offset),
            span,
        );

        let data_ptr_rvalue = Rvalue::BinaryOp(
            BinOp::Add,
            Box::new((
                Operand::Copy(Place::new(data_ptr_ptr)),
                Operand::Copy(Place::new(byte_offset_local)),
            )),
        );
        self.push_stmt(
            StatementKind::Assign(Place::new(data_ptr_local), data_ptr_rvalue),
            span,
        );

        let new_len_rvalue = Rvalue::BinaryOp(
            BinOp::Sub,
            Box::new((end_val, Operand::Copy(Place::new(start_local)))),
        );
        self.push_stmt(
            StatementKind::Assign(Place::new(new_len_local), new_len_rvalue),
            span,
        );

        let slice_operands = vec![
            Operand::Copy(Place::new(data_ptr_local)),
            Operand::Copy(Place::new(new_len_local)),
        ];
        Rvalue::Aggregate(glyim_mir::AggregateKind::Tuple, slice_operands)
    }
}
