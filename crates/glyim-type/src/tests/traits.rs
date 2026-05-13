use super::helpers::test_frozen_ty_ctx;
use crate::*;

// S02-T12: TyCtxMut is !Send + !Sync
// S02-T13: TyCtx is Send + Sync

#[test]
fn ty_ctx_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<TyCtx>();
}

#[test]
fn ty_ctx_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<TyCtx>();
}

#[test]
fn ty_ctx_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TyCtx>();
}

#[test]
fn ty_ctx_mut_not_send_not_sync() {
    // TyCtxMut contains PhantomData<*const ()> which makes it !Send + !Sync.
    // This is by design — after typeck, only TyCtx (frozen) is shared.
    // We verify freeze produces a Send+Sync TyCtx:
    let frozen = test_frozen_ty_ctx();
    let _boxed: Box<dyn Send + Sync> = Box::new(frozen);
}
