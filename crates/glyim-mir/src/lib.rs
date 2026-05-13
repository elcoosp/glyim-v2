use glyim_core::arena::IndexVec;
use glyim_core::def_id::DefId;
use glyim_type::Ty;

glyim_core::define_idx!(BasicBlockIdx);
glyim_core::define_idx!(LocalIdx);

#[derive(Clone, Debug)]
pub struct Body {
    pub owner: DefId,
    pub locals: IndexVec<LocalIdx, LocalDecl>,
    pub return_ty: Ty,
}

#[derive(Clone, Debug)]
pub struct LocalDecl {
    pub ty: Ty,
}

impl Body {
    pub fn dummy(owner: DefId) -> Self {
        use glyim_type::Ty;
        let locals = IndexVec::new();
        Self {
            owner,
            locals,
            return_ty: Ty::ERROR,
        }
    }
}
