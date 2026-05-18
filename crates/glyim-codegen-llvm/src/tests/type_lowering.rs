use crate::LlvmBackend;
use glyim_core::Interner;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::Mutability;
use glyim_mir::{
    BasicBlockData, Body, LocalDecl, LocalIdx, MirConst, MirConstKind, Operand, Place, Rvalue,
    SourceInfo, Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_span::{ByteIdx, FileId, Span};
use glyim_type::{TyCtxMut, TyKind};

fn build_tuple_body(ctx: &mut TyCtxMut) -> Body {
    let i32_ty = ctx.mk_ty(TyKind::Int(glyim_core::primitives::IntTy::I32));
    let tuple_subst = ctx.intern_substitution(vec![
        glyim_type::GenericArg::Ty(i32_ty),
        glyim_type::GenericArg::Ty(i32_ty),
    ]);
    let tuple_ty = ctx.mk_ty(TyKind::Tuple(tuple_subst));

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl {
        ty: tuple_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::new(
            FileId::BOGUS,
            ByteIdx::ZERO,
            ByteIdx::ZERO,
            glyim_span::SyntaxContext::ROOT,
        )),
    });
    locals.push(LocalDecl {
        ty: i32_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: i32_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });

    let const_10 = MirConst {
        kind: MirConstKind::Int(10),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    let const_20 = MirConst {
        kind: MirConstKind::Int(20),
        ty: i32_ty,
        span: Span::DUMMY,
    };

    let mut basic_blocks: IndexVec<glyim_mir::BasicBlockIdx, BasicBlockData> = IndexVec::new();

    let stmts = vec![
        Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::Use(Operand::Constant(const_10)),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        },
        Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(2)),
                Rvalue::Use(Operand::Constant(const_20)),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        },
        Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Aggregate(
                    glyim_mir::AggregateKind::Tuple,
                    vec![
                        Operand::Copy(Place::new(LocalIdx::from_raw(1))),
                        Operand::Copy(Place::new(LocalIdx::from_raw(2))),
                    ],
                ),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        },
    ];

    let mut bb0 = BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    bb0.statements = stmts;
    basic_blocks.push(bb0);

    Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty: tuple_ty,
        span: Span::DUMMY,
        var_debug_info: Vec::new(),
    }
}

#[test]
fn u05_t01_struct_type_lowered_to_llvm_struct() {
    let mut ctx = TyCtxMut::new(Interner::default());
    let body = build_tuple_body(&mut ctx);
    let frozen = ctx.freeze();

    let backend = LlvmBackend::new().with_ty_ctx(frozen);
    let ir = backend
        .generate_ir(&body)
        .expect("generate_ir should succeed");

    assert!(
        ir.contains("{ i32, i32 }"),
        "IR should contain struct type {{ i32, i32 }}, got:\n{}",
        ir
    );
    assert!(
        ir.contains("insertvalue"),
        "IR should contain insertvalue for tuple construction, got:\n{}",
        ir
    );
}

#[test]
fn u05_t01_struct_type_ir_is_valid() {
    let mut ctx = TyCtxMut::new(Interner::default());
    let body = build_tuple_body(&mut ctx);
    let frozen = ctx.freeze();

    let backend = LlvmBackend::new().with_ty_ctx(frozen);
    let result = backend.generate_ir(&body);
    assert!(
        result.is_ok(),
        "generate_ir should succeed: {:?}",
        result.err()
    );
}
