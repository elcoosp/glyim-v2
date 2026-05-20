use glyim_core::def_id::CrateId;
use glyim_core::interner::Interner;
use glyim_vfs::Vfs;
use parking_lot::RwLock;

pub struct Database {
    interner: Interner,
    vfs: Vfs,
    _ty_ctx: RwLock<Option<glyim_type::TyCtx>>,
    krate: CrateId,
    _config: CrateConfig,
    /// Cache of previously computed mono item symbols.
    /// Allows reusing monomorphization results across compilations
    /// with the same Database instance.
    _mono_cache: RwLock<Option<Vec<String>>>,
}

#[derive(Clone, Debug)]
pub struct CrateConfig {
    pub name: String,
    pub target_triple: String,
    pub opt_level: u8,
}

impl Database {
    /// Obtain a mutable reference to the interner.
    pub fn intern_mut(&mut self) -> &mut Interner {
        &mut self.interner
    }

    pub fn new(config: CrateConfig) -> Self {
        Self {
            interner: Interner::new(),
            vfs: Vfs::new(),
            _ty_ctx: RwLock::new(None),
            krate: CrateId::from_raw(0),
            _config: config,
            _mono_cache: RwLock::new(None),
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
        *self._ty_ctx.write() = Some(ctx);
    }

    pub fn ty_ctx(&self) -> parking_lot::RwLockReadGuard<'_, Option<glyim_type::TyCtx>> {
        self._ty_ctx.read()
    }

    /// Store mono item symbols in the cache for potential reuse.
    pub fn set_mono_cache(&self, items: Vec<String>) {
        *self._mono_cache.write() = Some(items);
    }

    /// Retrieve the cached mono item symbols from a previous compilation.
    pub fn mono_cache(&self) -> parking_lot::RwLockReadGuard<'_, Option<Vec<String>>> {
        self._mono_cache.read()
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
