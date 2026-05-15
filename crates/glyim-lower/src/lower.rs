use glyim_core::arena::IndexVec;
use glyim_core::def_id::AdtId;
use glyim_core::interner::Name;
use glyim_core::primitives::Mutability;
use glyim_diag::GlyimDiagnostic;
use glyim_mir::{self, BasicBlockIdx, LocalIdx};
use glyim_mir::{CastKind, ProjectionElem};
use glyim_span::Span;
use glyim_type::FieldIdx;
use glyim_type::*;
use glyim_typeck::thir;

#[derive(Clone, Debug)]
pub struct LowerResult {
    pub body: glyim_mir::Body,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

pub trait LowerCtx {
    fn ty_ctx(&self) -> &TyCtx;
    fn adt_def(&self, id: AdtId) -> AdtDef;
    fn push_span(&self, span: Span);
    fn pop_span(&self);
}

pub struct AdtDef {
    pub variants: Vec<AdtVariant>,
    pub kind: AdtKind,
}

pub struct AdtVariant {
    pub fields: Vec<Ty>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdtKind {
    Struct,
    Enum,
    Union,
}

#[allow(dead_code)]
struct MirBuilder<'a> {
    _ctx: &'a dyn LowerCtx,
    locals: IndexVec<LocalIdx, glyim_mir::LocalDecl>,
    basic_blocks: IndexVec<BasicBlockIdx, glyim_mir::BasicBlockData>,
    arg_count: usize,
    return_ty: Ty,
    owner: glyim_core::def_id::DefId,
    span: Span,
    diagnostics: Vec<GlyimDiagnostic>,
    var_map: std::collections::HashMap<Name, LocalIdx>,

    current_block: Option<BasicBlockIdx>,
}

impl<'a> MirBuilder<'a> {
    fn new(ctx: &'a dyn LowerCtx, thir: &thir::Body) -> Self {
        let mut locals = IndexVec::new();
        // _0 is return place
        locals.push(glyim_mir::LocalDecl {
            ty: thir.return_ty,
            mutability: Mutability::Mut,
            source_info: glyim_mir::SourceInfo::new(thir.span),
        });

        Self {
            _ctx: ctx,
            locals,
            basic_blocks: IndexVec::new(),
            arg_count: thir.params.len(),
            return_ty: thir.return_ty,
            owner: thir.owner,
            span: thir.span,
            diagnostics: Vec::new(),
            var_map: std::collections::HashMap::new(),
            current_block: None,
        }
    }

    fn new_block(&mut self) -> BasicBlockIdx {
        self.basic_blocks.push(glyim_mir::BasicBlockData {
            statements: Vec::new(),
            terminator: glyim_mir::Terminator {
                kind: glyim_mir::TerminatorKind::Unreachable,
                source_info: glyim_mir::SourceInfo::new(self.span),
            },
            is_cleanup: false,
        })
    }

    fn alloc_local(&mut self, ty: Ty, mutability: Mutability, span: Span) -> LocalIdx {
        self.locals.push(glyim_mir::LocalDecl {
            ty,
            mutability,
            source_info: glyim_mir::SourceInfo::new(span),
        })
    }

    fn push_stmt(&mut self, stmt: glyim_mir::StatementKind, span: Span) {
        if let Some(bb) = self.current_block {
            self.basic_blocks[bb].statements.push(glyim_mir::Statement {
                kind: stmt,
                source_info: glyim_mir::SourceInfo::new(span),
            });
        }
    }

    fn terminate(&mut self, kind: glyim_mir::TerminatorKind, span: Span) {
        if let Some(bb) = self.current_block {
            self.basic_blocks[bb].terminator = glyim_mir::Terminator {
                kind,
                source_info: glyim_mir::SourceInfo::new(span),
            };
            self.current_block = None;
        }
    }

    fn lower_body(&mut self, thir: &thir::Body) {
        let entry = self.new_block();
        self.current_block = Some(entry);

        for param in &thir.params {
            let local = self.alloc_local(param.ty, Mutability::Not, param.span);
            if let thir::PatternKind::Binding { name, .. } = &param.pat.kind {
                self.var_map.insert(*name, local);
            }
        }

        for stmt in &thir.stmts {
            self.lower_stmt(stmt);
        }

        if self.current_block.is_some() {
            self.terminate(glyim_mir::TerminatorKind::Return, thir.span);
        }
    }

    fn lower_stmt(&mut self, stmt: &thir::Stmt) {
        match stmt {
            thir::Stmt::Let {
                name,
                ty,
                init,
                span,
                pat,
                ..
            } => {
                // If there is a pattern, stub destructuring (no access to pattern storage)
                // pat is a thir::Pattern, not Option; we ignore it for now
                let _ = pat;
                tracing::warn!("STUB: pattern destructuring not implemented (pattern ignored)");
                let local = self.alloc_local(*ty, Mutability::Mut, *span);
                self.var_map.insert(*name, local);
                self.push_stmt(glyim_mir::StatementKind::StorageLive(local), *span);

                if let Some(init_expr) = init {
                    let rvalue = self.lower_expr_to_rvalue(init_expr);
                    self.push_stmt(
                        glyim_mir::StatementKind::Assign(glyim_mir::Place::new(local), rvalue),
                        *span,
                    );
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
                let _ = self.lower_expr_to_rvalue(expr);
                tracing::warn!("STUB: expr stmt dropped");
            }
        }
    }

    fn lower_expr_to_rvalue(&mut self, expr: &thir::Expr) -> glyim_mir::Rvalue {
        match &expr.kind {
            thir::ExprKind::Literal(lit) => {
                let mir_const = match lit {
                    thir::Literal::Int(val, _) => glyim_mir::MirConst {
                        kind: glyim_mir::MirConstKind::Int(*val),
                        ty: expr.ty,
                        span: expr.span,
                    },
                    thir::Literal::Uint(val, _) => glyim_mir::MirConst {
                        kind: glyim_mir::MirConstKind::Uint(*val),
                        ty: expr.ty,
                        span: expr.span,
                    },
                    thir::Literal::Bool(val) => glyim_mir::MirConst {
                        kind: glyim_mir::MirConstKind::Bool(*val),
                        ty: expr.ty,
                        span: expr.span,
                    },
                    _ => {
                        tracing::warn!("STUB: unsupported literal");
                        glyim_mir::MirConst {
                            kind: glyim_mir::MirConstKind::Error,
                            ty: expr.ty,
                            span: expr.span,
                        }
                    }
                };
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(mir_const))
            }
            thir::ExprKind::VarRef(var_id) => {
                let local = LocalIdx::from_raw(var_id.to_raw());
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(glyim_mir::Place::new(local)))
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
            thir::ExprKind::FnRef(_) => {
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Error,
                    ty: expr.ty,
                    span: expr.span,
                }))
            }
            thir::ExprKind::Match { scrutinee, arms } => {
                let discr_op = self.lower_expr_to_operand(scrutinee);
                let mut targets = Vec::new();
                let merge_bb = self.new_block();
                let dest_local = self.alloc_local(expr.ty, Mutability::Mut, expr.span);
                let dest_place = glyim_mir::Place::new(dest_local);

                let mut arm_blocks: Vec<(BasicBlockIdx, &thir::MatchArm)> = Vec::new();
                for (i, arm) in arms.iter().enumerate() {
                    let arm_bb = self.new_block();
                    if i < arms.len() - 1 {
                        let val = match &arm.pat.kind {
                            thir::PatternKind::Literal(lit) => match lit {
                                thir::Literal::Int(v, _) => *v as u128,
                                thir::Literal::Uint(v, _) => *v,
                                thir::Literal::Bool(b) => *b as u128,
                                _ => u128::MAX,
                            },
                            _ => u128::MAX,
                        };
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
                    expr.span,
                );

                for (arm_bb, arm) in arm_blocks.iter() {
                    self.current_block = Some(*arm_bb);
                    if let Some(_guard) = &arm.guard {
                        tracing::warn!("STUB: match guards");
                    }
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

                self.current_block = Some(merge_bb);
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Move(dest_place))
            }
            thir::ExprKind::While { cond, body } => {
                let header_bb = self.new_block();
                let body_bb = self.new_block();
                let exit_bb = self.new_block();

                // Jump to header
                self.terminate(
                    glyim_mir::TerminatorKind::Goto { target: header_bb },
                    expr.span,
                );

                // Header: evaluate condition and branch
                self.current_block = Some(header_bb);
                let cond_op = self.lower_expr_to_operand(cond);
                let targets = glyim_mir::SwitchTargets::new(
                    Box::new([(1, body_bb)]), // true -> body
                    exit_bb,                  // false -> exit
                );
                self.terminate(
                    glyim_mir::TerminatorKind::SwitchInt {
                        discr: cond_op,
                        switch_ty: cond.ty,
                        targets,
                    },
                    cond.span,
                );

                // Body block
                self.current_block = Some(body_bb);
                let _body_rval = self.lower_expr_to_rvalue(body);
                // after body, jump back to header
                self.terminate(
                    glyim_mir::TerminatorKind::Goto { target: header_bb },
                    body.span,
                );

                self.current_block = Some(exit_bb);
                // while produces unit value
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Unit,
                    ty: Ty::UNIT,
                    span: expr.span,
                }))
            }
            thir::ExprKind::Loop { body } => {
                let loop_bb = self.new_block();
                self.terminate(
                    glyim_mir::TerminatorKind::Goto { target: loop_bb },
                    expr.span,
                );

                self.current_block = Some(loop_bb);
                let _ = self.lower_expr_to_rvalue(body);
                // Infinite loop: jump back to itself (unless break inserted, not handled)
                self.terminate(
                    glyim_mir::TerminatorKind::Goto { target: loop_bb },
                    body.span,
                );

                // Loop never exits; unreachable after.
                // Return value is never, but we'll use a dummy
                self.current_block = Some(self.new_block());
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Error,
                    ty: Ty::NEVER,
                    span: expr.span,
                }))
            }
            thir::ExprKind::Field {
                receiver,
                field: _field,
                ty: field_ty,
            } => {
                let base_place = self.lower_expr_to_place(receiver);
                // Look up AdtDef to find field index
                let adt_id = match self._ctx.ty_ctx().ty_kind(receiver.ty) {
                    TyKind::Adt(adt_id, _) => *adt_id,
                    _ => {
                        tracing::warn!("STUB: field access on non-ADT type");
                        return glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(
                            glyim_mir::MirConst {
                                kind: glyim_mir::MirConstKind::Error,
                                ty: *field_ty,
                                span: expr.span,
                            },
                        ));
                    }
                };
                let adt_def = self._ctx.adt_def(adt_id);
                let variant = &adt_def.variants[0]; // assume single variant for struct
                let field_idx = variant
                    .fields
                    .iter()
                    .position(|_f| {
                        // match by name? we need field name; adt_variant fields are Ty, not named.
                        // Cannot resolve name, stub with index 0
                        tracing::warn!("STUB: field name lookup not implemented");
                        false
                    })
                    .unwrap_or(0);
                let projection = {
                    let mut proj = base_place.projection.to_vec();
                    proj.push(ProjectionElem::Field(FieldIdx::from_raw(field_idx as u32)));
                    proj.into_boxed_slice()
                };
                let place = glyim_mir::Place {
                    local: base_place.local,
                    projection,
                };
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
                let projection = {
                    let mut proj = base_place.projection.to_vec();
                    proj.push(ProjectionElem::Index(index_local));
                    proj.into_boxed_slice()
                };
                let place = glyim_mir::Place {
                    local: base_place.local,
                    projection,
                };
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(place))
            }
            thir::ExprKind::Cast { expr: inner } => {
                let operand = self.lower_expr_to_operand(inner);
                // Determine CastKind simplistically
                let inner_ty = inner.ty;
                let target_ty = expr.ty; // overall cast expression type
                let cast_kind = match (
                    self._ctx.ty_ctx().ty_kind(inner_ty),
                    self._ctx.ty_ctx().ty_kind(target_ty),
                ) {
                    (TyKind::Int(_), TyKind::Int(_)) => CastKind::IntToInt,
                    (TyKind::Float(_), TyKind::Int(_)) => CastKind::FloatToInt,
                    (TyKind::Int(_), TyKind::Float(_)) => CastKind::IntToFloat,
                    _ => CastKind::PtrToPtr, // dummy
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
            _ => {
                tracing::warn!("STUB: unhandled expr kind");
                glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(glyim_mir::MirConst {
                    kind: glyim_mir::MirConstKind::Error,
                    ty: expr.ty,
                    span: expr.span,
                }))
            }
        }
    }

    fn lower_expr_to_operand(&mut self, expr: &thir::Expr) -> glyim_mir::Operand {
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

    fn lower_expr_to_place(&mut self, expr: &thir::Expr) -> glyim_mir::Place {
        match &expr.kind {
            thir::ExprKind::VarRef(var_id) => {
                let local = LocalIdx::from_raw(var_id.to_raw());
                glyim_mir::Place::new(local)
            }
            _ => {
                tracing::warn!("STUB: unhandled place expr");
                glyim_mir::Place::new(LocalIdx::from_raw(0))
            }
        }
    }
}

pub fn lower_body(ctx: &dyn LowerCtx, thir: &thir::Body) -> LowerResult {
    let mut builder = MirBuilder::new(ctx, thir);
    builder.lower_body(thir);

    let mut body = glyim_mir::Body::dummy(builder.owner);
    body.basic_blocks = builder.basic_blocks;
    body.locals = builder.locals;
    body.arg_count = builder.arg_count;
    body.return_ty = builder.return_ty;
    body.span = builder.span;

    LowerResult {
        body,
        diagnostics: builder.diagnostics,
    }
}
