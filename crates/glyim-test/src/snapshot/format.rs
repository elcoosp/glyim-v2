use glyim_type::TyCtx;

pub fn format_mir_body(ctx: &TyCtx, body: &glyim_mir::Body) -> String {
    let mut out = String::new();
    out.push_str(&format!("fn {}():\n", body.owner));
    out.push_str("  locals:\n");
    for (idx, local) in body.locals.iter_enumerated() {
        let mut_str = match local.mutability {
            glyim_core::primitives::Mutability::Mut => "mut",
            glyim_core::primitives::Mutability::Not => "imm",
        };
        out.push_str(&format!(
            "    ${}: {} ({})\n",
            idx.to_raw(),
            glyim_type::PrintTy::new(local.ty, ctx),
            mut_str,
        ));
    }
    for (idx, block) in body.basic_blocks.iter_enumerated() {
        out.push_str(&format!("  bb{}:\n", idx.to_raw()));
        for stmt in &block.statements {
            out.push_str(&format!("    {:?}\n", stmt.kind));
        }
        out.push_str(&format!("    -> {:?}\n", block.terminator.kind));
    }
    out
}

pub fn format_def_map(def_map: &glyim_def_map::CrateDefMap) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "CrateDefMap (root: {:?}, krate: {:?})\n",
        def_map.root, def_map.krate
    ));
    for (idx, module) in def_map.modules.iter_enumerated() {
        out.push_str(&format!("  module {:?}:\n", idx));
        out.push_str(&format!("    parent: {:?}\n", module.parent));
        out.push_str(&format!("    children: {}\n", module.children.len()));
    }
    out
}
