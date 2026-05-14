use crate::{BorrowckCtx, check_borrows, extract_constraints};
use glyim_core::arena::IndexVec;
use glyim_core::primitives::Mutability;
use glyim_core::{CrateId, DefId, LocalDefId};
use glyim_mir::{
    BasicBlockData, BasicBlockIdx, Body, BorrowKind, LocalDecl, LocalIdx, Operand, Place, Rvalue,
    SourceInfo, Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_span::Span;
use glyim_test::with_fresh_ty_ctx;
use glyim_type::Ty;

// Local mock implementing the crate's BorrowckCtx
struct LocalMockBorrowckCtx {
    ty_ctx: glyim_type::TyCtx,
    body: Body,
}

impl BorrowckCtx for LocalMockBorrowckCtx {
    fn ty_ctx(&self) -> &glyim_type::TyCtx {
        &self.ty_ctx
    }
    fn local_decl(&self, local: glyim_mir::LocalIdx) -> &glyim_mir::LocalDecl {
        &self.body.locals[local]
    }
    fn is_copy(&self, _ty: Ty) -> bool {
        false
    }
}

fn make_body(statements: Vec<Statement>, locals: Vec<LocalDecl>) -> Body {
    let def_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    let mut body = Body::dummy(def_id);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    })]);
    body.basic_blocks[BasicBlockIdx::from_raw(0)].statements = statements;
    body.locals = IndexVec::from_raw(locals);
    body
}

fn local_decl(ty: Ty) -> LocalDecl {
    LocalDecl {
        ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

fn assign_borrow(dest: LocalIdx, place: Place, kind: BorrowKind) -> Statement {
    Statement {
        kind: StatementKind::Assign(Place::new(dest), Rvalue::Ref(place, kind)),
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

fn use_local(dest: LocalIdx, local: LocalIdx) -> Statement {
    Statement {
        kind: StatementKind::Assign(
            Place::new(dest),
            Rvalue::Use(Operand::Copy(Place::new(local))),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

fn make_ref_ty(ctx_mut: &mut glyim_type::TyCtxMut, inner: Ty, mutable: bool) -> Ty {
    let mutability = if mutable {
        Mutability::Mut
    } else {
        Mutability::Not
    };
    ctx_mut.mk_ref(glyim_type::Region::Erased, inner, mutability)
}

#[test]
fn t01_no_borrows_no_errors() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let locals = vec![local_decl(unit)];
        make_body(vec![], locals)
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(result.errors.is_empty());
}

#[test]
fn t02_two_shared_borrows_no_error() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let shared_ref1 = make_ref_ty(ctx_mut, unit, false);
        let shared_ref2 = make_ref_ty(ctx_mut, unit, false);
        let locals = vec![
            local_decl(unit),
            local_decl(shared_ref1),
            local_decl(shared_ref2),
            local_decl(unit),
        ];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(1),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Shared,
                ),
                assign_borrow(
                    LocalIdx::from_raw(2),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Shared,
                ),
                use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(1)),
                use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(2)),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(result.errors.is_empty());
}

#[test]
fn t03_shared_and_mutable_conflict_error() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let shared_ref = make_ref_ty(ctx_mut, unit, false);
        let mut_ref = make_ref_ty(ctx_mut, unit, true);
        let locals = vec![
            local_decl(unit),
            local_decl(shared_ref),
            local_decl(mut_ref),
            local_decl(unit),
        ];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(1),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Shared,
                ),
                assign_borrow(
                    LocalIdx::from_raw(2),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(1)),
                use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(2)),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(!result.errors.is_empty(), "Expected borrow conflict error");
}

#[test]
fn t04_two_mutable_borrows_conflict_error() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let mut_ref1 = make_ref_ty(ctx_mut, unit, true);
        let mut_ref2 = make_ref_ty(ctx_mut, unit, true);
        let locals = vec![
            local_decl(unit),
            local_decl(mut_ref1),
            local_decl(mut_ref2),
            local_decl(unit),
        ];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(1),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                assign_borrow(
                    LocalIdx::from_raw(2),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(1)),
                use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(2)),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(!result.errors.is_empty(), "Expected borrow conflict error");
}

#[test]
fn t05_borrow_expires_after_last_use_no_error() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let mut_ref1 = make_ref_ty(ctx_mut, unit, true);
        let mut_ref2 = make_ref_ty(ctx_mut, unit, true);
        let locals = vec![
            local_decl(unit),
            local_decl(mut_ref1),
            local_decl(unit),
            local_decl(mut_ref2),
        ];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(1),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                use_local(LocalIdx::from_raw(2), LocalIdx::from_raw(1)),
                assign_borrow(
                    LocalIdx::from_raw(3),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(
        result.errors.is_empty(),
        "Expected no error; borrow expired"
    );
}

#[test]
fn t06_region_constraints_extracted() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let shared_ref = make_ref_ty(ctx_mut, unit, false);
        let locals = vec![local_decl(unit), local_decl(shared_ref), local_decl(unit)];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(1),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Shared,
                ),
                use_local(LocalIdx::from_raw(2), LocalIdx::from_raw(1)),
            ],
            locals,
        )
    });
    let constraints = extract_constraints(&ctx, &body);
    assert!(
        !constraints.is_empty(),
        "Expected at least one region constraint"
    );
}

#[test]
fn t07_error_diagnostics_include_span() {
    let span1 = Span::new(
        glyim_span::FileId::BOGUS,
        glyim_span::ByteIdx::from_raw(0),
        glyim_span::ByteIdx::from_raw(5),
        glyim_span::SyntaxContext::ROOT,
    );
    let span2 = Span::new(
        glyim_span::FileId::BOGUS,
        glyim_span::ByteIdx::from_raw(10),
        glyim_span::ByteIdx::from_raw(15),
        glyim_span::SyntaxContext::ROOT,
    );
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let mut_ref1 = make_ref_ty(ctx_mut, unit, true);
        let mut_ref2 = make_ref_ty(ctx_mut, unit, true);
        let locals = vec![
            local_decl(unit),
            local_decl(mut_ref1),
            local_decl(mut_ref2),
            local_decl(unit),
        ];
        make_body(
            vec![
                Statement {
                    kind: StatementKind::Assign(
                        Place::new(LocalIdx::from_raw(1)),
                        Rvalue::Ref(
                            Place::new(LocalIdx::from_raw(0)),
                            BorrowKind::Mut {
                                allow_two_phase_borrow: false,
                            },
                        ),
                    ),
                    source_info: SourceInfo::new(span1),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::new(LocalIdx::from_raw(2)),
                        Rvalue::Ref(
                            Place::new(LocalIdx::from_raw(0)),
                            BorrowKind::Mut {
                                allow_two_phase_borrow: false,
                            },
                        ),
                    ),
                    source_info: SourceInfo::new(span2),
                },
                use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(1)),
                use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(2)),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(!result.errors.is_empty());
    let has_span = result.errors.iter().any(|d| d.span.primary != Span::DUMMY);
    assert!(
        has_span,
        "Expected at least one error with a non-dummy span"
    );
}

#[test]
fn t08_local_ty_via_borrowck_ctx() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let locals = vec![local_decl(unit)];
        make_body(vec![], locals)
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let decl = mock.local_decl(LocalIdx::from_raw(0));
    assert_eq!(decl.ty, Ty::UNIT);
}

#[test]
fn t09_multiple_conflicts_in_one_body() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let mut_ref_ty = make_ref_ty(ctx_mut, unit, true);
        let locals = vec![
            local_decl(unit),
            local_decl(unit),
            local_decl(mut_ref_ty),
            local_decl(mut_ref_ty),
            local_decl(unit),
        ];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(2),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                assign_borrow(
                    LocalIdx::from_raw(3),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                assign_borrow(
                    LocalIdx::from_raw(2),
                    Place::new(LocalIdx::from_raw(1)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
                use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    // Should have at least 1 error (first conflict on local 0)
    assert!(
        !result.errors.is_empty(),
        "Expected at least one borrow conflict"
    );
}

#[test]
fn t10_shared_borrow_after_mutable_expires_no_error() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let mut_ref = make_ref_ty(ctx_mut, unit, true);
        let shared_ref = make_ref_ty(ctx_mut, unit, false);
        let locals = vec![
            local_decl(unit),
            local_decl(mut_ref),
            local_decl(unit),
            local_decl(shared_ref),
            local_decl(unit),
        ];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(1),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                use_local(LocalIdx::from_raw(2), LocalIdx::from_raw(1)),
                assign_borrow(
                    LocalIdx::from_raw(3),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Shared,
                ),
                use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(
        result.errors.is_empty(),
        "Expected no error: mutable borrow expired before shared"
    );
}

#[test]
fn t11_mutable_borrow_after_shared_expires_no_error() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let shared_ref = make_ref_ty(ctx_mut, unit, false);
        let mut_ref = make_ref_ty(ctx_mut, unit, true);
        let locals = vec![
            local_decl(unit),
            local_decl(shared_ref),
            local_decl(unit),
            local_decl(mut_ref),
            local_decl(unit),
        ];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(1),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Shared,
                ),
                use_local(LocalIdx::from_raw(2), LocalIdx::from_raw(1)),
                assign_borrow(
                    LocalIdx::from_raw(3),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(
        result.errors.is_empty(),
        "Expected no error: shared borrow expired before mutable"
    );
}

#[test]
fn t12_borrow_of_different_locals_no_conflict() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let mut_ref1 = make_ref_ty(ctx_mut, unit, true);
        let mut_ref2 = make_ref_ty(ctx_mut, unit, true);
        let locals = vec![
            local_decl(unit),
            local_decl(unit),
            local_decl(mut_ref1),
            local_decl(mut_ref2),
            local_decl(unit),
        ];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(2),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                assign_borrow(
                    LocalIdx::from_raw(3),
                    Place::new(LocalIdx::from_raw(1)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
                use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(
        result.errors.is_empty(),
        "Expected no error: borrows of different locals"
    );
}

#[test]
fn t13_error_message_contains_borrow_kind_info() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let mut_ref1 = make_ref_ty(ctx_mut, unit, true);
        let mut_ref2 = make_ref_ty(ctx_mut, unit, true);
        let locals = vec![
            local_decl(unit),
            local_decl(mut_ref1),
            local_decl(mut_ref2),
            local_decl(unit),
        ];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(1),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                assign_borrow(
                    LocalIdx::from_raw(2),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(1)),
                use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(2)),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(!result.errors.is_empty());
    let msg = &result.errors[0].message;
    assert!(
        msg.contains("mutable"),
        "Error message should mention 'mutable': {}",
        msg
    );
    assert!(
        msg.contains("borrow"),
        "Error message should mention 'borrow': {}",
        msg
    );
}

#[test]
fn t14_extract_constraints_includes_spans() {
    let span1 = Span::new(
        glyim_span::FileId::BOGUS,
        glyim_span::ByteIdx::from_raw(42),
        glyim_span::ByteIdx::from_raw(47),
        glyim_span::SyntaxContext::ROOT,
    );
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let shared_ref = make_ref_ty(ctx_mut, unit, false);
        let locals = vec![local_decl(unit), local_decl(shared_ref), local_decl(unit)];
        make_body(
            vec![
                Statement {
                    kind: StatementKind::Assign(
                        Place::new(LocalIdx::from_raw(1)),
                        Rvalue::Ref(Place::new(LocalIdx::from_raw(0)), BorrowKind::Shared),
                    ),
                    source_info: SourceInfo::new(span1),
                },
                use_local(LocalIdx::from_raw(2), LocalIdx::from_raw(1)),
            ],
            locals,
        )
    });
    let constraints = extract_constraints(&ctx, &body);
    assert!(!constraints.is_empty());
    let has_span = constraints.iter().any(|c| c.span == span1);
    assert!(
        has_span,
        "Constraint should carry the span from the borrow statement"
    );
}

#[test]
fn t15_no_borrow_check_for_copy_types_ignored() {
    // Even if a type is Copy, our current implementation still tracks borrows
    // because the MIR has explicit Ref statements. This test verifies that
    // the analysis runs without panicking on Copy types; it doesn't skip them.
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::primitives::IntTy::I32));
        let shared_ref = make_ref_ty(ctx_mut, i32_ty, false);
        let locals = vec![
            local_decl(i32_ty),
            local_decl(shared_ref),
            local_decl(i32_ty),
            local_decl(shared_ref),
            local_decl(i32_ty),
        ];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(1),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Shared,
                ),
                use_local(LocalIdx::from_raw(2), LocalIdx::from_raw(1)),
                assign_borrow(
                    LocalIdx::from_raw(3),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Shared,
                ),
                use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    // Multiple shared borrows should be fine
    assert!(result.errors.is_empty());
}

#[test]
fn t16_empty_body_no_panics() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        make_body(vec![], vec![local_decl(unit)])
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(result.errors.is_empty());
}

#[test]
fn t17_borrow_dest_never_used_expires_immediately() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let mut_ref1 = make_ref_ty(ctx_mut, unit, true);
        let mut_ref2 = make_ref_ty(ctx_mut, unit, true);
        let locals = vec![local_decl(unit), local_decl(mut_ref1), local_decl(mut_ref2)];
        make_body(
            vec![
                assign_borrow(
                    LocalIdx::from_raw(1),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
                assign_borrow(
                    LocalIdx::from_raw(2),
                    Place::new(LocalIdx::from_raw(0)),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: false,
                    },
                ),
            ],
            locals,
        )
    });
    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert!(
        result.errors.is_empty(),
        "Borrows never used expire immediately, no conflict"
    );
}

#[test]
fn t18_is_copy_delegates_to_ctx() {
    struct CopyMockCtx {
        ty_ctx: glyim_type::TyCtx,
        body: Body,
    }
    impl BorrowckCtx for CopyMockCtx {
        fn ty_ctx(&self) -> &glyim_type::TyCtx {
            &self.ty_ctx
        }
        fn local_decl(&self, local: glyim_mir::LocalIdx) -> &glyim_mir::LocalDecl {
            &self.body.locals[local]
        }
        fn is_copy(&self, _ty: Ty) -> bool {
            true
        }
    }

    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        make_body(vec![], vec![local_decl(unit)])
    });
    let mock = CopyMockCtx { ty_ctx: ctx, body };
    assert!(mock.is_copy(Ty::UNIT));
}
