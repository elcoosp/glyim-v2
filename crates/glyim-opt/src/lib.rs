use std::sync::Arc;
use glyim_mir::Body;
use glyim_type::TyCtx;
pub struct Optimized { pub body: Body }
pub fn optimize(_ctx: &TyCtx, _body: &Arc<Body>) -> Optimized { todo!("STUB: optimize") }
