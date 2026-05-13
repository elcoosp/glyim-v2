use glyim_type::TyCtx;

pub fn format_mir_body(ctx: &TyCtx, body: &glyim_mir::Body) -> String {
    let mut out = String::new();
    out.push_str(&format!("fn {}():\n", body.owner));
    out.push_str(&format!("  arg_count: {}\n", body.arg_count));
    out.push_str(&format!("  return_ty: {}\n", glyim_type::PrintTy::new(body.return_ty, ctx)));
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
    if !body.var_debug_info.is_empty() {
        out.push_str("  debug:\n");
        for info in &body.var_debug_info {
            out.push_str(&format!("    {:?}: ", info.name));
            match &info.value {
                glyim_mir::VarDebugInfoValue::Place(place) => {
                    out.push_str(&format!("{}\n", format_place(place)));
                }
                glyim_mir::VarDebugInfoValue::Const(c) => {
                    out.push_str(&format!("const {}\n", format_const(c)));
                }
            }
        }
    }
    for (idx, block) in body.basic_blocks.iter_enumerated() {
        let cleanup_tag = if block.is_cleanup { " (cleanup)" } else { "" };
        out.push_str(&format!("  bb{}:\n", idx.to_raw()));
        for stmt in &block.statements {
            out.push_str(&format!("    {}\n", format_statement(&stmt.kind)));
        }
        out.push_str(&format!("    -> {}\n", format_terminator(&block.terminator.kind)));
    }
    out
}

fn format_statement(kind: &glyim_mir::StatementKind) -> String {
    match kind {
        glyim_mir::StatementKind::Assign(place, rvalue) => {
            format!("{} = {}", format_place(place), format_rvalue(rvalue))
        }
        glyim_mir::StatementKind::StorageLive(local) => {
            format!("StorageLive(${})", local.to_raw())
        }
        glyim_mir::StatementKind::StorageDead(local) => {
            format!("StorageDead(${})", local.to_raw())
        }
        glyim_mir::StatementKind::Nop => "Nop".to_string(),
    }
}

fn format_rvalue(rvalue: &glyim_mir::Rvalue) -> String {
    match rvalue {
        glyim_mir::Rvalue::Use(op) => format!("Use({})", format_operand(op)),
        glyim_mir::Rvalue::Ref(place, bk) => {
            format!("Ref({}, {})", format_borrow_kind(bk), format_place(place))
        }
        glyim_mir::Rvalue::BinaryOp(op, pair) => {
            format!("BinaryOp({:?}, {}, {})", op, format_operand(&pair.0), format_operand(&pair.1))
        }
        glyim_mir::Rvalue::UnaryOp(op, val) => {
            format!("UnaryOp({:?}, {})", op, format_operand(val))
        }
        glyim_mir::Rvalue::Aggregate(kind, ops) => {
            let args: Vec<String> = ops.iter().map(format_operand).collect();
            format!("Aggregate({}, [{}])", format_aggregate_kind(kind), args.join(", "))
        }
        glyim_mir::Rvalue::Discriminant(place) => {
            format!("Discriminant({})", format_place(place))
        }
        glyim_mir::Rvalue::Len(place) => {
            format!("Len({})", format_place(place))
        }
        glyim_mir::Rvalue::Cast(kind, op, ty) => {
            format!("Cast({}, {}, {:?})", format_cast_kind(kind), format_operand(op), ty)
        }
        glyim_mir::Rvalue::Repeat(op, count) => {
            format!("Repeat({}, {})", format_operand(op), format_const(count))
        }
    }
}

fn format_aggregate_kind(kind: &glyim_mir::AggregateKind) -> String {
    match kind {
        glyim_mir::AggregateKind::Array(ty) => format!("Array({:?})", ty),
        glyim_mir::AggregateKind::Tuple => "Tuple".to_string(),
        glyim_mir::AggregateKind::Adt(adt_id, variant, substs) => {
            format!("Adt({:?}, variant={:?}, substs={:?})", adt_id, variant.to_raw(), substs)
        }
        glyim_mir::AggregateKind::Closure(closure_id, substs) => {
            format!("Closure({:?}, substs={:?})", closure_id, substs)
        }
    }
}

fn format_borrow_kind(bk: &glyim_mir::BorrowKind) -> String {
    match bk {
        glyim_mir::BorrowKind::Shared => "Shared".to_string(),
        glyim_mir::BorrowKind::Unique => "Unique".to_string(),
        glyim_mir::BorrowKind::Mut { allow_two_phase_borrow } => {
            format!("Mut(two_phase={})", allow_two_phase_borrow)
        }
    }
}

fn format_cast_kind(kind: &glyim_mir::CastKind) -> String {
    match kind {
        glyim_mir::CastKind::IntToInt => "IntToInt".to_string(),
        glyim_mir::CastKind::FloatToInt => "FloatToInt".to_string(),
        glyim_mir::CastKind::IntToFloat => "IntToFloat".to_string(),
        glyim_mir::CastKind::PtrToPtr => "PtrToPtr".to_string(),
        glyim_mir::CastKind::FnPtrToPtr => "FnPtrToPtr".to_string(),
    }
}

fn format_terminator(kind: &glyim_mir::TerminatorKind) -> String {
    match kind {
        glyim_mir::TerminatorKind::Goto { target } => {
            format!("Goto(bb{})", target.to_raw())
        }
        glyim_mir::TerminatorKind::SwitchInt { discr, switch_ty, targets } => {
            let branches: Vec<String> = targets.iter()
                .map(|(val, bb)| format!("{}: bb{}", val, bb.to_raw()))
                .collect();
            format!(
                "SwitchInt(discr={}, ty={:?}, [{}], otherwise=bb{})",
                format_operand(discr),
                switch_ty,
                branches.join(", "),
                targets.otherwise().to_raw()
            )
        }
        glyim_mir::TerminatorKind::Return => "Return".to_string(),
        glyim_mir::TerminatorKind::Unreachable => "Unreachable".to_string(),
        glyim_mir::TerminatorKind::Call { func, args, destination, target, cleanup } => {
            let args_fmt: Vec<String> = args.iter().map(format_operand).collect();
            let target_fmt = match target {
                Some(bb) => format!("Some(bb{})", bb.to_raw()),
                None => "None".to_string(),
            };
            let cleanup_fmt = match cleanup {
                Some(bb) => format!("Some(bb{})", bb.to_raw()),
                None => "None".to_string(),
            };
            format!(
                "Call(func={}, args=[{}], dest={}, target={}, cleanup={})",
                format_operand(func),
                args_fmt.join(", "),
                format_place(destination),
                target_fmt,
                cleanup_fmt,
            )
        }
        glyim_mir::TerminatorKind::Assert { cond, expected, target, cleanup, msg } => {
            let cleanup_fmt = match cleanup {
                Some(bb) => format!("Some(bb{})", bb.to_raw()),
                None => "None".to_string(),
            };
            format!(
                "Assert(cond={}, expected={}, target=bb{}, cleanup={}, msg={})",
                format_operand(cond),
                expected,
                target.to_raw(),
                cleanup_fmt,
                format_assert_message(msg),
            )
        }
        glyim_mir::TerminatorKind::Drop { place, target, cleanup } => {
            let cleanup_fmt = match cleanup {
                Some(bb) => format!("Some(bb{})", bb.to_raw()),
                None => "None".to_string(),
            };
            format!(
                "Drop({}, target=bb{}, cleanup={})",
                format_place(place),
                target.to_raw(),
                cleanup_fmt,
            )
        }
    }
}

fn format_assert_message(msg: &glyim_mir::AssertMessage) -> String {
    match msg {
        glyim_mir::AssertMessage::Overflow(op) => format!("Overflow({:?})", op),
        glyim_mir::AssertMessage::DivisionByZero => "DivisionByZero".to_string(),
        glyim_mir::AssertMessage::RemainderByZero => "RemainderByZero".to_string(),
        glyim_mir::AssertMessage::BoundsCheck => "BoundsCheck".to_string(),
    }
}

fn format_place(place: &glyim_mir::Place) -> String {
    let base = format!("${}", place.local.to_raw());
    if place.projection.is_empty() {
        base
    } else {
        let proj: Vec<String> = place.projection.iter().map(format_projection_elem).collect();
        format!("{}.{}", base, proj.join("."))
    }
}

fn format_projection_elem(elem: &glyim_mir::ProjectionElem) -> String {
    match elem {
        glyim_mir::ProjectionElem::Deref => "Deref".to_string(),
        glyim_mir::ProjectionElem::Field(idx) => format!("Field({})", idx.to_raw()),
        glyim_mir::ProjectionElem::Index(idx) => format!("Index(${})", idx.to_raw()),
        glyim_mir::ProjectionElem::Downcast(variant) => format!("Downcast(variant={})", variant.to_raw()),
    }
}

fn format_operand(op: &glyim_mir::Operand) -> String {
    match op {
        glyim_mir::Operand::Copy(place) => format!("Copy({})", format_place(place)),
        glyim_mir::Operand::Move(place) => format!("Move({})", format_place(place)),
        glyim_mir::Operand::Constant(c) => format!("Const({})", format_const(c)),
    }
}

fn format_const(c: &glyim_mir::MirConst) -> String {
    match &c.kind {
        glyim_mir::MirConstKind::Int(v) => format!("int({})", v),
        glyim_mir::MirConstKind::Uint(v) => format!("uint({})", v),
        glyim_mir::MirConstKind::FloatBits(bits) => format!("float_bits({})", bits),
        glyim_mir::MirConstKind::Bool(v) => format!("bool({})", v),
        glyim_mir::MirConstKind::Char(v) => format!("char('{}')", v.escape_default()),
        glyim_mir::MirConstKind::String(name) => format!("string({:?})", name),
        glyim_mir::MirConstKind::Unit => "()".to_string(),
        glyim_mir::MirConstKind::Error => "<error>".to_string(),
    }
}

pub fn format_def_map(def_map: &glyim_def_map::CrateDefMap) -> String {
    let mut out = String::new();
    out.push_str(&format!("CrateDefMap (root: {:?}, krate: {:?})\n", def_map.root, def_map.krate));
    for (idx, module) in def_map.modules.iter_enumerated() {
        out.push_str(&format!("  module {:?}:\n", idx));
        out.push_str(&format!("    parent: {:?}\n", module.parent));
        out.push_str(&format!("    origin: {}\n", format_module_origin(&module.origin)));
        out.push_str(&format!("    children: {}\n", module.children.len()));
        out.push_str(&format!("    scope.types: {}\n", module.scope.types.len()));
        out.push_str(&format!("    scope.values: {}\n", module.scope.values.len()));
        out.push_str(&format!("    scope.macros: {}\n", module.scope.macros.len()));
    }
    out
}

fn format_module_origin(origin: &glyim_def_map::ModuleOrigin) -> String {
    match origin {
        glyim_def_map::ModuleOrigin::CrateRoot => "CrateRoot".to_string(),
        glyim_def_map::ModuleOrigin::File { file_id } => format!("File({:?})", file_id),
        glyim_def_map::ModuleOrigin::Inline { span } => format!("Inline({:?})", span),
    }
}
