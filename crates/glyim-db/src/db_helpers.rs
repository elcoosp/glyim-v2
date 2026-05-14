use glyim_core::interner::Interner;

use super::Database;

/// Public helper to obtain a mutable reference to the interner from a Database.
/// This is needed by the pipeline for HIR lowering, which requires mutable access.
pub fn intern_mut(db: &mut Database) -> &mut Interner {
    &mut db.interner
}
