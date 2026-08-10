use crate::lower_terminator::TerminatorExt;
use glyim_core::arena::IndexVec;
use glyim_core::interner::Name;
use glyim_core::primitives::Mutability;
use glyim_diag::GlyimDiagnostic;
use glyim_mir::{BasicBlockIdx, LocalIdx};
use glyim_span::Span;
use glyim_type::*;
use glyim_typeck::thir;

use crate::lower::LowerCtx;

/// Information about an enclosing loop, used for break/continue targeting.
pub(crate) struct LoopInfo {
    pub(crate) continue_bb: BasicBlockIdx,
    pub(crate) break_bb: BasicBlockIdx,
}

/// The MIR Builder.
///
/// Accumulates locals, basic blocks, statements, and terminators as THIR
/// expressions are recursively lowered to MIR.
pub struct MirBuilder<'a> {
    pub(crate) ctx: &'a dyn LowerCtx,
    pub(crate) locals: IndexVec<LocalIdx, glyim_mir::LocalDecl>,
    pub(crate) basic_blocks: IndexVec<BasicBlockIdx, glyim_mir::BasicBlockData>,
    pub(crate) arg_count: usize,
    pub(crate) return_ty: Ty,
    pub(crate) owner: glyim_core::def_id::DefId,
    pub(crate) span: Span,
    pub(crate) diagnostics: Vec<GlyimDiagnostic>,
    pub(crate) closure_bodies: Vec<(glyim_core::def_id::ClosureId, glyim_type::Substitution, glyim_mir::Body)>,
    pub(crate) var_map: std::collections::HashMap<Name, LocalIdx>,
    pub(crate) current_block: Option<BasicBlockIdx>,
    /// Stack of enclosing loops for break/continue resolution.
    pub(crate) loop_stack: Vec<LoopInfo>,
}

impl<'a> MirBuilder<'a> {
    /// Create a new MIR builder for the given THIR body.
    pub fn new(ctx: &'a dyn LowerCtx, thir: &thir::Body) -> Self {
        let mut locals = IndexVec::new();
        // _0 is return place
        locals.push(glyim_mir::LocalDecl {
            ty: thir.return_ty,
            mutability: Mutability::Mut,
            source_info: glyim_mir::SourceInfo::new(thir.span),
        });

        Self {
            ctx,
            locals,
            basic_blocks: IndexVec::new(),
            arg_count: thir.params.len(),
            return_ty: thir.return_ty,
            owner: thir.owner,
            span: thir.span,
            diagnostics: Vec::new(),
            closure_bodies: Vec::new(),
            var_map: std::collections::HashMap::new(),
            current_block: None,
            loop_stack: Vec::new(),
        }
    }

    /// Allocate a new basic block and return its index.
    pub fn new_block(&mut self) -> BasicBlockIdx {
        self.basic_blocks.push(glyim_mir::BasicBlockData {
            statements: Vec::new(),
            terminator: glyim_mir::Terminator {
                kind: glyim_mir::TerminatorKind::Unreachable,
                source_info: glyim_mir::SourceInfo::new(self.span),
            },
            is_cleanup: false,
        })
    }

    /// Allocate a new local variable and return its index.
    pub fn alloc_local(&mut self, ty: Ty, mutability: Mutability, span: Span) -> LocalIdx {
        self.locals.push(glyim_mir::LocalDecl {
            ty,
            mutability,
            source_info: glyim_mir::SourceInfo::new(span),
        })
    }

    /// Push a statement onto the current basic block.
    pub fn push_stmt(&mut self, stmt: glyim_mir::StatementKind, span: Span) {
        if let Some(bb) = self.current_block {
            self.basic_blocks[bb].statements.push(glyim_mir::Statement {
                kind: stmt,
                source_info: glyim_mir::SourceInfo::new(span),
            });
        }
    }

    /// Lower a THIR body into this builder, populating locals, blocks, etc.
    pub fn lower_body(&mut self, thir: &thir::Body) {
        let entry = self.new_block();
        self.current_block = Some(entry);

        for param in &thir.params {
            let local = self.alloc_local(param.ty, Mutability::Not, param.span);
            self.push_stmt(glyim_mir::StatementKind::StorageLive(local), param.span);
            match &param.pat.kind {
                thir::PatternKind::Binding {
                    name,
                    mutability: _,
                    subpattern,
                } => {
                    self.var_map.insert(*name, local);
                    if let Some(sub) = subpattern {
                        self.bind_pattern(sub, Some(local), param.span);
                    }
                }
                thir::PatternKind::Wild => {}
                _ => {
                    self.bind_pattern(&param.pat, Some(local), param.span);
                }
            }
        }

        for stmt in &thir.stmts {
            self.lower_stmt(stmt);
        }

        if self.current_block.is_some() {
            self.terminate(glyim_mir::TerminatorKind::Return, thir.span);
        }
    }

    /// Lower a closure expression: generate its MIR body and return an aggregate.

    
    
    /// Lower a closure expression: generate its MIR body and return the aggregate.
    
    pub(crate) fn lower_closure(
        &mut self,
        thir_body: &thir::Body,
        captures: &[thir::Capture],
        closure_id: glyim_core::def_id::ClosureId,
        substs: glyim_type::Substitution,
        span: glyim_span::Span,
    ) -> glyim_mir::Rvalue {
        use glyim_core::def_id::{CrateId, DefId, LocalDefId};
        use glyim_mir::{BasicBlockData, BasicBlockIdx, Rvalue, TerminatorKind, LocalDecl, Operand, Place, Statement, StatementKind, SourceInfo};
        use glyim_typeck::thir;
        use std::collections::HashMap;

        // Create DefId for the closure.
        let def_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(closure_id.to_raw()));

        // Build a new MIR body for the closure.
        let mut closure_body = glyim_mir::Body::dummy(def_id);
        closure_body.return_ty = thir_body.return_ty;
        closure_body.span = span;

        // Map original LocalVarId to MIR LocalIdx.
        let mut var_map: HashMap<thir::LocalVarId, glyim_mir::LocalIdx> = HashMap::new();

        // Add capture parameters.
        for capture in captures {
            let mutability = match capture.kind {
                thir::CaptureKind::ByValue => glyim_core::primitives::Mutability::Not,
                thir::CaptureKind::ByRef(m) => m,
            };
            let local = closure_body.locals.push(LocalDecl {
                ty: capture.ty,
                mutability,
                source_info: SourceInfo::new(span),
            });
            var_map.insert(capture.local, local);
        }

        // Add original parameters.
        for param in &thir_body.params {
            let mutability = match &param.pat.kind {
                thir::PatternKind::Binding { mutability, .. } => *mutability,
                _ => glyim_core::primitives::Mutability::Not,
            };
            let local = closure_body.locals.push(LocalDecl {
                ty: param.ty,
                mutability,
                source_info: SourceInfo::new(param.span),
            });
            // We need to map the parameter's LocalVarId. We don't have it here.
            // Instead, we'll rely on the fact that the THIR body's params are in order.
            // We'll store them in a vector by index.
        }

        // Set arg_count.
        closure_body.arg_count = captures.len() + thir_body.params.len();

        // To lower the body, we need to create a MirBuilder for the closure body
        // with a custom context that maps VarRefs to the new locals.
        // Since we have var_map from capture local to MIR local, we need to
        // rewrite the THIR body to replace capture VarRefs with the mapped locals.
        // However, the THIR body is immutable; we can clone and transform.
        // But for simplicity, we'll implement a custom lowering that uses the
        // existing MirBuilder logic but with a different mapping.

        // We'll create a new MirBuilder using the closure body and a custom LowerCtx.
        // This is complex, so for now we'll keep the placeholder body.
        // TODO: implement full lowering.

        // For now, create a placeholder body that returns unit.
        let entry = BasicBlockIdx::from_raw(0);
        let block = BasicBlockData {
            statements: vec![],
            terminator: glyim_mir::Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(span),
            },
            is_cleanup: false,
        };
        closure_body.basic_blocks.push(block);

        // Store the closure body.
        self.closure_bodies.push((closure_id, substs, closure_body));

        // Build the aggregate: function pointer + captures.
        let fn_const = glyim_mir::MirConst {
            kind: glyim_mir::MirConstKind::Fn(
                glyim_core::def_id::FnDefId::from_raw(closure_id.to_raw()),
                substs,
            ),
            ty: self.ctx.ty_ctx().error_ty(), // placeholder
            span,
        };
        let fn_operand = glyim_mir::Operand::Constant(fn_const);
        let mut operands = vec![fn_operand];
        for capture in captures {
            let local = glyim_mir::LocalIdx::from_raw(capture.local.to_raw());
            let operand = match capture.kind {
                thir::CaptureKind::ByValue => glyim_mir::Operand::Move(glyim_mir::Place::new(local)),
                thir::CaptureKind::ByRef(_) => glyim_mir::Operand::Copy(glyim_mir::Place::new(local)),
            };
            operands.push(operand);
        }
        Rvalue::Aggregate(glyim_mir::AggregateKind::Closure(closure_id, substs), operands)
    }




    
}