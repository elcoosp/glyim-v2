use crate::interner::Name;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathKind {
    Plain,
    SelfPath,
    Super(u32),
    Crate,
}

/// A single segment of a path, e.g. the `Bar<T, U>` in `foo::Bar<T, U>::baz`.
///
/// Plan §1.1: each segment can carry its own generic arguments. Previously the
/// core `Path` could only express a bare `Name` per segment, forcing HIR to
/// keep a parallel richer path type and duplicate conversion logic everywhere.
/// Storing `generic_args` here (as the list of written type-parameter names)
/// lets a single `Path` represent the general case — `foo::Bar<T, U>::baz`
/// round-trips with each segment's args intact.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PathSegment {
    pub name: Name,
    /// The generic arguments written at this segment, if any. `None` means no
    /// `<...>` was written; `Some(vec![])` means `<>` (empty, e.g. turbofish).
    pub generic_args: Option<Vec<Name>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Path {
    pub segments: Vec<PathSegment>,
    pub kind: PathKind,
}

impl Path {
    /// A single-segment path with no generic arguments.
    pub fn from_single(name: Name) -> Self {
        Self {
            segments: vec![PathSegment {
                name,
                generic_args: None,
            }],
            kind: PathKind::Plain,
        }
    }

    /// Build a path from an explicit list of segments (plan §1.1).
    pub fn from_segments(segments: Vec<PathSegment>) -> Self {
        Self {
            segments,
            kind: PathKind::Plain,
        }
    }

    /// Convenience: a single-segment path carrying generic arguments.
    pub fn from_single_with_args(name: Name, generic_args: Vec<Name>) -> Self {
        Self {
            segments: vec![PathSegment {
                name,
                generic_args: Some(generic_args),
            }],
            kind: PathKind::Plain,
        }
    }

    pub fn as_name(&self) -> Option<Name> {
        if self.segments.len() == 1 && self.kind == PathKind::Plain {
            Some(self.segments[0].name)
        } else {
            None
        }
    }
}
