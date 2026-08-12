use glyim_core::def_id::CrateId;
use glyim_core::interner::Interner;
use glyim_vfs::Vfs;
use parking_lot::RwLock;
use std::sync::Arc;

/// Handle to share the TyCtx between the Database and CodegenBackend.
pub type TyCtxHandle = Arc<std::sync::RwLock<Option<Arc<glyim_type::TyCtx>>>>;

pub struct Database {
    interner: Interner,
    vfs: Vfs,
    ty_ctx: TyCtxHandle,
    krate: CrateId,
    /// Cache of previously computed mono item symbols.
    mono_cache: RwLock<Option<Vec<String>>>,
}

#[derive(Clone, Debug)]
pub struct CrateConfig {
    pub name: String,
    pub target_triple: String,
    pub opt_level: u8,
}

impl Database {
    pub fn intern_mut(&mut self) -> &mut Interner {
        &mut self.interner
    }

    pub fn new(_config: CrateConfig) -> Self {
        Self {
            interner: Interner::new(),
            vfs: Vfs::new(),
            ty_ctx: Arc::new(std::sync::RwLock::new(None)),
            krate: CrateId::from_raw(0),
            mono_cache: RwLock::new(None),
        }
    }

    pub fn interner(&self) -> &Interner {
        &self.interner
    }

    pub fn vfs(&self) -> &Vfs {
        &self.vfs
    }

    pub fn krate(&self) -> CrateId {
        self.krate
    }

    pub fn set_ty_ctx(&self, ctx: glyim_type::TyCtx) {
        *self.ty_ctx.write().unwrap() = Some(Arc::new(ctx));
    }

    pub fn get_ty_ctx(&self) -> Option<Arc<glyim_type::TyCtx>> {
        self.ty_ctx.read().unwrap().clone()
    }

    pub fn ty_ctx_handle(&self) -> TyCtxHandle {
        self.ty_ctx.clone()
    }

    pub fn set_mono_cache(&self, items: Vec<String>) {
        *self.mono_cache.write() = Some(items);
    }

    pub fn mono_cache(&self) -> parking_lot::RwLockReadGuard<'_, Option<Vec<String>>> {
        self.mono_cache.read()
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

#[cfg(test)]
mod tests;
