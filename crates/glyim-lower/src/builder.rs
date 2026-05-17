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

/// The MIR Builder.
pub struct MirBuilder<'a> {
    pub(crate) _ctx: &'a dyn LowerCtx,
    pub(crate) locals: IndexVec<LocalIdx, glyim_mir::LocalDecl>,
    pub(crate) basic_blocks: IndexVec<BasicBlockIdx, glyim_mir::BasicBlockData>,
    pub(crate) arg_count: usize,
    pub(crate) return_ty: Ty,
    pub(crate) owner: glyim_core::def_id::DefId,
    pub(crate) span: Span,
    pub(crate) diagnostics: Vec<GlyimDiagnostic>,
    pub(crate) var_map: std::collections::HashMap<Name, LocalIdx>,

    pub(crate) current_block: Option<BasicBlockIdx>,
}

impl<'a> MirBuilder<'a> {
    pub fn new(ctx: &'a dyn LowerCtx, thir: &thir::Body) -> Self {
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

    pub fn alloc_local(&mut self, ty: Ty, mutability: Mutability, span: Span) -> LocalIdx {
        self.locals.push(glyim_mir::LocalDecl {
            ty,
            mutability,
            source_info: glyim_mir::SourceInfo::new(span),
        })
    }

    pub fn push_stmt(&mut self, stmt: glyim_mir::StatementKind, span: Span) {
        if let Some(bb) = self.current_block {
            self.basic_blocks[bb].statements.push(glyim_mir::Statement {
                kind: stmt,
                source_info: glyim_mir::SourceInfo::new(span),
            });
        }
    }

    pub fn lower_body(&mut self, thir: &thir::Body) {
        let entry = self.new_block();
        self.current_block = Some(entry);

        for param in &thir.params {
            let local = self.alloc_local(param.ty, Mutability::Not, param.span);
            if let thir::PatternKind::Binding { name, .. } = &param.pat.kind {
                self.var_map.insert(*name, local);
            }
        }

        for stmt in &thir.stmts {
            // Delegated to lower_rvalue module
            self.lower_stmt(stmt);
        }

        if self.current_block.is_some() {
            // Delegated to lower_terminator module
            self.terminate(glyim_mir::TerminatorKind::Return, thir.span);
        }
    }
}
