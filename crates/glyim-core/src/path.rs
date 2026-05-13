use crate::interner::Name;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathKind {
    Plain,
    SelfPath,
    Super(u32),
    Crate,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PathSegment {
    pub name: Name,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Path {
    pub segments: Vec<PathSegment>,
    pub kind: PathKind,
}

impl Path {
    pub fn from_single(name: Name) -> Self {
        Self { segments: vec![PathSegment { name }], kind: PathKind::Plain }
    }

    pub fn as_name(&self) -> Option<Name> {
        if self.segments.len() == 1 && self.kind == PathKind::Plain {
            Some(self.segments[0].name)
        } else {
            None
        }
    }
}
