//! VTable layout computation tests

use crate::vtable::*;
use crate::*;
use glyim_core::primitives::*;
use glyim_test::with_fresh_ty_ctx;

#[test]
fn s15_vtable_memory_size_no_methods() {
    let (_ctx, concrete_ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let layout = VTableLayout {
        trait_def_id: glyim_core::TraitDefId::from_raw(1),
        concrete_ty,
        size: Size::bytes(1),
        align: Align::ONE,
        drop_fn: None,
        methods: vec![],
    };
    let mem = layout.memory_size(8);
    assert_eq!(mem.size, 24, "3 pointers * 8 bytes = 24");
    assert_eq!(mem.align, 8);
}

#[test]
fn s15_vtable_memory_size_with_methods() {
    let (ctx, (concrete_ty, sig, foo_name, bar_name)) = with_fresh_ty_ctx(|c| {
        let i32_ty = c.mk_ty(glyim_type::TyKind::Int(IntTy::I32));
        let bool_ty = c.bool_ty();
        let inputs = c.intern_substitution(vec![glyim_type::GenericArg::Ty(i32_ty)]);
        let sig = glyim_type::FnSig {
            inputs,
            output: bool_ty,
            c_variadic: false,
            unsafety: Safety::Safe,
            abi: Abi::Glyim,
        };
        let foo_name = c.resolver().intern("foo");
        let bar_name = c.resolver().intern("bar");
        (bool_ty, sig, foo_name, bar_name)
    });
    let layout = VTableLayout {
        trait_def_id: glyim_core::TraitDefId::from_raw(1),
        concrete_ty,
        size: Size::bytes(1),
        align: Align::ONE,
        drop_fn: None,
        methods: vec![
            VTableEntry {
                name: foo_name,
                sig: sig.clone(),
                fn_def_id: glyim_core::FnDefId::from_raw(10),
            },
            VTableEntry {
                name: bar_name,
                sig,
                fn_def_id: glyim_core::FnDefId::from_raw(11),
            },
        ],
    };
    let mem = layout.memory_size(8);
    assert_eq!(mem.size, 40, "5 pointers * 8 = 40");
    assert_eq!(mem.align, 8);
}

#[test]
fn s15_vtable_method_offset() {
    let (_ctx, concrete_ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let layout = VTableLayout {
        trait_def_id: glyim_core::TraitDefId::from_raw(1),
        concrete_ty,
        size: Size::bytes(1),
        align: Align::ONE,
        drop_fn: None,
        methods: vec![],
    };
    assert_eq!(layout.method_offset(0, 8), 24);
    assert_eq!(layout.method_offset(1, 8), 32);
    assert_eq!(layout.method_offset(0, 4), 12);
}

#[test]
fn s15_vtable_computer_trait() {
    let (ctx, concrete_ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let computer = SimpleLayoutComputer::new(&ctx, TargetInfo::x86_64());
    let result = computer.vtable_of(glyim_core::TraitDefId::from_raw(1), concrete_ty);
    assert!(result.is_some(), "vtable_of should return Some");
    let vtable = result.unwrap();
    assert_eq!(vtable.trait_def_id, glyim_core::TraitDefId::from_raw(1));
    assert_eq!(vtable.size, Size::bytes(1));
    assert_eq!(vtable.align, Align::ONE);
}
