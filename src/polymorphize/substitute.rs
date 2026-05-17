//! Substitution manipulation for polymorphization.

use glyim_type::*;

/// Replace unused parameters in a substitution with a canonical placeholder.
///
/// Unused type parameters become `Ty::UNIT`, unused const parameters become
/// `ConstKind::Unit`. Lifetime parameters are left unchanged.
///
/// This produces a "polymorphized" substitution that can be used as a
/// deduplication key: two items with different original substitutions but
/// the same polymorphized substitution can share a single mono item.
pub fn polymorphize_substs(
    ctx: &mut TyCtxMut,
    substs: Substitution,
    used: &[bool],
) -> Substitution {
    let args: Vec<GenericArg> = ctx
        .substitution_args(substs)
        .iter()
        .enumerate()
        .map(|(i, arg)| {
            if i < used.len() && !used[i] {
                match arg {
                    GenericArg::Ty(_) => GenericArg::Ty(ctx.unit_ty()),
                    GenericArg::Lifetime(r) => GenericArg::Lifetime(r.clone()),
                    GenericArg::Const(_) => GenericArg::Const(Const {
                        kind: ConstKind::Unit,
                        ty: ctx.unit_ty(),
                    }),
                }
            } else {
                arg.clone()
            }
        })
        .collect();
    ctx.intern_substitution(args)
}
