//! Analysis of generic parameter usage in MIR bodies.

use glyim_mir::{
    self, AggregateKind, MirConstKind, Operand, Rvalue, StatementKind, TerminatorKind,
};
use glyim_type::*;

/// Analyze which parameters in a substitution are actually used in a MIR body.
///
/// Returns a boolean vector where `used[i]` is `true` if the parameter at
/// position `i` in the substitution appears in the body's types.
///
/// A parameter is considered "used" if any `TyKind::Param(ParamTy { index: i, .. })`
/// or `ConstKind::Param(ParamConst { index: i, .. })` appears in the body's
/// local types, rvalues, operands, or terminators.
pub fn analyze_used_params(
    body: &glyim_mir::Body,
    ctx: &dyn TypeLookup,
    substs: Substitution,
) -> Vec<bool> {
    let n = ctx.substitution_args(substs).len();
    let mut used = vec![false; n];

    // Check locals
    for local in body.locals.iter() {
        mark_used_params(local.ty, ctx, &mut used);
    }

    // Check statements and terminators
    for block in body.basic_blocks.iter() {
        for stmt in &block.statements {
            if let StatementKind::Assign(_, ref rvalue) = stmt.kind {
                mark_used_params_in_rvalue(rvalue, ctx, &mut used);
            }
        }
        mark_used_params_in_terminator(&block.terminator.kind, ctx, &mut used);
    }

    used
}

// ---- Internal helpers for parameter usage analysis ----

/// Walk a type and mark any `TyKind::Param` or const params as used.
fn mark_used_params(ty: Ty, ctx: &dyn TypeLookup, used: &mut [bool]) {
    match ctx.ty_kind(ty) {
        TyKind::Param(ParamTy { index, .. }) => {
            let i = *index as usize;
            if i < used.len() {
                used[i] = true;
            }
        }
        TyKind::Ref(_, inner, _) | TyKind::RawPtr(inner, _) => {
            mark_used_params(*inner, ctx, used);
        }
        TyKind::Slice(inner) => {
            mark_used_params(*inner, ctx, used);
        }
        TyKind::Array(inner, _) => {
            mark_used_params(*inner, ctx, used);
        }
        TyKind::Tuple(substs)
        | TyKind::Adt(_, substs)
        | TyKind::Closure(_, substs)
        | TyKind::Opaque(_, substs) => {
            mark_used_params_in_subst(*substs, ctx, used);
        }
        TyKind::FnDef(_, substs) => {
            mark_used_params_in_subst(*substs, ctx, used);
        }
        TyKind::FnPtr(sig) => {
            for arg in ctx.substitution_args(sig.inputs) {
                if let GenericArg::Ty(t) = arg {
                    mark_used_params(*t, ctx, used);
                }
            }
            mark_used_params(sig.output, ctx, used);
        }
        TyKind::Dynamic(binder, _) => {
            for pred in binder.clone().skip_binder().iter() {
                mark_used_params_in_predicate(pred, ctx, used);
            }
        }
        TyKind::Projection(proj) => {
            mark_used_params_in_subst(proj.trait_ref.substs, ctx, used);
        }
        _ => {}
    }
}

/// Walk substitution arguments and mark used params.
fn mark_used_params_in_subst(substs: Substitution, ctx: &dyn TypeLookup, used: &mut [bool]) {
    for arg in ctx.substitution_args(substs) {
        match arg {
            GenericArg::Ty(t) => mark_used_params(*t, ctx, used),
            GenericArg::Lifetime(_) => {}
            GenericArg::Const(c) => mark_used_params_in_const(c, ctx, used),
        }
    }
}

/// Walk a `glyim_type::Const` and mark used params.
fn mark_used_params_in_const(c: &Const, ctx: &dyn TypeLookup, used: &mut [bool]) {
    if let ConstKind::Param(ParamConst { index, .. }) = &c.kind {
        let i = *index as usize;
        if i < used.len() {
            used[i] = true;
        }
    }
    mark_used_params(c.ty, ctx, used);
}

/// Walk a `Predicate` and mark used params.
fn mark_used_params_in_predicate(pred: &Predicate, ctx: &dyn TypeLookup, used: &mut [bool]) {
    match pred {
        Predicate::Trait(tp) => {
            mark_used_params_in_subst(tp.trait_ref.substs, ctx, used);
        }
        Predicate::TypeOutlives(top) => {
            mark_used_params(top.ty, ctx, used);
        }
        Predicate::RegionOutlives(_) => {}
        Predicate::WellFormed(ty) => {
            mark_used_params(*ty, ctx, used);
        }
        Predicate::Coerce(a, b) => {
            mark_used_params(*a, ctx, used);
            mark_used_params(*b, ctx, used);
        }
    }
}

/// Walk a `MirConst` and mark used params.
fn mark_used_params_in_mir_const(c: &glyim_mir::MirConst, ctx: &dyn TypeLookup, used: &mut [bool]) {
    mark_used_params(c.ty, ctx, used);
    match &c.kind {
        MirConstKind::Fn(_, substs) | MirConstKind::ConstRef(_, substs) => {
            mark_used_params_in_subst(*substs, ctx, used);
        }
        _ => {}
    }
}

/// Walk an `Operand` and mark used params.
fn mark_used_params_in_operand(op: &Operand, ctx: &dyn TypeLookup, used: &mut [bool]) {
    if let Operand::Constant(c) = op {
        mark_used_params_in_mir_const(c, ctx, used);
    }
    // Copy/Move operands reference places whose types are already checked via locals
}

/// Walk an `Rvalue` and mark used params.
fn mark_used_params_in_rvalue(rv: &Rvalue, ctx: &dyn TypeLookup, used: &mut [bool]) {
    match rv {
        Rvalue::Use(op) => mark_used_params_in_operand(op, ctx, used),
        Rvalue::BinaryOp(_, boxed) => {
            let (l, r) = boxed.as_ref();
            mark_used_params_in_operand(l, ctx, used);
            mark_used_params_in_operand(r, ctx, used);
        }
        Rvalue::UnaryOp(_, op) => mark_used_params_in_operand(op, ctx, used),
        Rvalue::Aggregate(kind, ops) => {
            match kind {
                AggregateKind::Array(ty) => mark_used_params(*ty, ctx, used),
                AggregateKind::Adt(_, _, substs) => mark_used_params_in_subst(*substs, ctx, used),
                AggregateKind::Closure(_, substs) => mark_used_params_in_subst(*substs, ctx, used),
                AggregateKind::Tuple => {}
            }
            for op in ops {
                mark_used_params_in_operand(op, ctx, used);
            }
        }
        Rvalue::Cast(_, op, ty) => {
            mark_used_params_in_operand(op, ctx, used);
            mark_used_params(*ty, ctx, used);
        }
        Rvalue::Repeat(op, mir_const) => {
            mark_used_params_in_operand(op, ctx, used);
            mark_used_params_in_mir_const(mir_const, ctx, used);
        }
        Rvalue::Ref(_, _) | Rvalue::Discriminant(_) | Rvalue::Len(_) => {}
    }
}

/// Walk a `TerminatorKind` and mark used params.
fn mark_used_params_in_terminator(kind: &TerminatorKind, ctx: &dyn TypeLookup, used: &mut [bool]) {
    match kind {
        TerminatorKind::Call { func, args, .. } => {
            mark_used_params_in_operand(func, ctx, used);
            for arg in args {
                mark_used_params_in_operand(arg, ctx, used);
            }
        }
        TerminatorKind::SwitchInt {
            discr, switch_ty, ..
        } => {
            mark_used_params_in_operand(discr, ctx, used);
            mark_used_params(*switch_ty, ctx, used);
        }
        TerminatorKind::Assert { cond, .. } => {
            mark_used_params_in_operand(cond, ctx, used);
        }
        TerminatorKind::Drop { .. } => {
            // Drop terminators reference places whose types are already
            // checked via the locals analysis. If T appears in a local that
            // is dropped, it's already marked as used. If T doesn't appear
            // in any local, there's nothing of type T to drop, so
            // polymorphization is safe.
        }
        TerminatorKind::Goto { .. } | TerminatorKind::Return | TerminatorKind::Unreachable => {}
    }
}
