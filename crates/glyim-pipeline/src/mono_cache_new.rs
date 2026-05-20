pub(crate) fn generate_drop_glue(ty: Ty, ty_ctx: &TyCtx) -> Arc<Body> {
    let def_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    let mut body = Body::dummy(def_id);
    body.return_ty = ty_ctx.unit_ty();

    let ptr_local = LocalIdx::from_raw(0);
    let place = Place::new(ptr_local);

    // Build a list of places to drop (flatten fields recursively)
    let mut drop_places = Vec::new();
    collect_drop_places(ty, &place, ty_ctx, &mut drop_places);

    if drop_places.is_empty() {
        // No drop needed
        if let Some(block) = body.basic_blocks.get_mut(BasicBlockIdx::from_raw(0)) {
            block.terminator.kind = TerminatorKind::Return;
        }
        return Arc::new(body);
    }

    // Create a basic block for each drop, chaining them with Goto.
    let start_block = BasicBlockIdx::from_raw(0);
    let mut prev_block = start_block;
    for (i, drop_place) in drop_places.iter().enumerate() {
        let next_block = if i == drop_places.len() - 1 {
            // Last drop: terminator returns
            None
        } else {
            Some(BasicBlockIdx::from_raw((i + 1) as u32))
        };
        let terminator = if let Some(target) = next_block {
            Terminator {
                kind: TerminatorKind::Drop {
                    place: drop_place.clone(),
                    target,
                    cleanup: None,
                },
                source_info: SourceInfo::new(Span::DUMMY),
            }
        } else {
            Terminator {
                kind: TerminatorKind::Drop {
                    place: drop_place.clone(),
                    target: BasicBlockIdx::from_raw(drop_places.len() as u32),
                    cleanup: None,
                },
                source_info: SourceInfo::new(Span::DUMMY),
            }
        };
        let block_data = BasicBlockData {
            statements: vec![],
            terminator,
            is_cleanup: false,
        };
        if i == 0 {
            *body.basic_blocks.get_mut(start_block).unwrap() = block_data;
        } else {
            body.basic_blocks.push(block_data);
        }
        prev_block = BasicBlockIdx::from_raw(i as u32);
    }

    // Add final return block
    let return_block = BasicBlockIdx::from_raw(drop_places.len() as u32);
    body.basic_blocks.push(BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    });

    // Fix the last drop's target to point to return_block
    let last_drop_idx = drop_places.len() - 1;
    if let Some(last_block) = body.basic_blocks.get_mut(BasicBlockIdx::from_raw(last_drop_idx as u32)) {
        if let TerminatorKind::Drop { target, .. } = &mut last_block.terminator.kind {
            *target = return_block;
        }
    }

    Arc::new(body)
}

fn collect_drop_places(ty: Ty, place: &Place, ty_ctx: &TyCtx, out: &mut Vec<Place>) {
    match ty_ctx.ty_kind(ty) {
        TyKind::Adt(adt_id, _) => {
            if let Some(adt_def) = ty_ctx.adt_def(*adt_id) {
                match adt_def.kind {
                    CoreAdtKind::Struct => {
                        for (field_idx, _) in adt_def.variants[0].fields.iter().enumerate() {
                            let mut proj = place.projection.to_vec();
                            proj.push(ProjectionElem::Field(FieldIdx::from_raw(field_idx as u32)));
                            let field_place = Place {
                                local: place.local,
                                projection: proj.into_boxed_slice(),
                            };
                            // Recurse to get inner drops
                            collect_drop_places(/* need field type */ ty_ctx.error_ty(), &field_place, ty_ctx, out);
                        }
                    }
                    CoreAdtKind::Enum => {
                        // For enums, we need to switch on discriminant. Simplified: just drop the whole place.
                        out.push(place.clone());
                    }
                    CoreAdtKind::Union => {}
                }
            } else {
                out.push(place.clone());
            }
        }
        TyKind::Array(elem_ty, _) | TyKind::Slice(elem_ty) => {
            // Drop each element. For now, just drop the whole array/slice.
            out.push(place.clone());
        }
        _ => {
            // Primitive types: no drop
        }
    }
}
