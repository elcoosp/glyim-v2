#[cfg(test)]
mod tests {
    use crate::*;
    use glyim_core::def_id::{FnDefId, AdtId};
    use glyim_test::test_ty_ctx;

    #[test]
    fn test_iterator_next_info_with_builtin() {
        let mut ctx_mut = test_ty_ctx();
        let mut trait_ctx = TraitContext::new();
        let next_def_id = FnDefId::from_raw(42);
        trait_ctx.set_builtin_iterator(BuiltinIteratorInfo { next_fn_def_id: next_def_id });
        let solver = SimpleTraitSolver::new(&trait_ctx);
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::primitives::IntTy::I32));
        let result = solver.iterator_next_info(&mut ctx_mut, i32_ty, i32_ty);
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.fn_def_id, next_def_id);
    }
}
