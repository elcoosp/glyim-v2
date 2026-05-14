use glyim_core::def_id::CrateId;
use glyim_core::interner::Interner;
use glyim_solve::TraitContext;
use glyim_vfs::Vfs;
use parking_lot::RwLock;

pub struct Database {
    interner: Interner,
    vfs: Vfs,
    _ty_ctx: RwLock<Option<glyim_type::TyCtx>>,
    trait_ctx: TraitContext,
    krate: CrateId,
    _config: CrateConfig, // renamed to _config
}

#[derive(Clone, Debug)]
pub struct CrateConfig {
    pub name: String,
    pub target_triple: String,
    pub opt_level: u8,
}

impl Database {
    /// Obtain a mutable reference to the interner.
    /// This should only be called during HIR lowering when no other
    /// references to the interner are held.
    pub fn intern_mut(&mut self) -> &mut Interner {
        &mut self.interner
    }
    pub fn new(config: CrateConfig) -> Self {
        Self {
            interner: Interner::new(),
            vfs: Vfs::new(),
            _ty_ctx: RwLock::new(None),
            trait_ctx: TraitContext::new(),
            krate: CrateId::from_raw(0),
            _config: config,
        }
    }

    pub fn interner(&self) -> &Interner {
        &self.interner
    }

    pub fn vfs(&self) -> &Vfs {
        &self.vfs
    }
    pub fn trait_ctx(&self) -> &TraitContext {
        &self.trait_ctx
    }
    pub fn krate(&self) -> CrateId {
        self.krate
    }

    pub fn set_ty_ctx(&self, ctx: glyim_type::TyCtx) {
        *self._ty_ctx.write() = Some(ctx);
    }
    pub fn ty_ctx(&self) -> parking_lot::RwLockReadGuard<'_, Option<glyim_type::TyCtx>> {
        self._ty_ctx.read()
    }
}

impl Default for Database {
    fn default() -> Self {
        Self::new(CrateConfig {
            name: "main".to_string(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            opt_level: 0,
        })
    }
}
pub mod db_helpers;
