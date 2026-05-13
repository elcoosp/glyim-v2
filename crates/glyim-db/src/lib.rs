use glyim_core::interner::Interner;
use glyim_vfs::Vfs;
use parking_lot::RwLock;
pub struct Database {
    interner: Interner,
    vfs: Vfs,
    _ty_ctx: RwLock<Option<glyim_type::TyCtx>>,
}
impl Database {
    pub fn new() -> Self {
        Self {
            interner: Interner::new(),
            vfs: Vfs::new(),
            _ty_ctx: RwLock::new(None),
        }
    }
    pub fn interner(&self) -> &Interner { &self.interner }
    pub fn vfs(&self) -> &Vfs { &self.vfs }
}
impl Default for Database { fn default() -> Self { Self::new() } }
