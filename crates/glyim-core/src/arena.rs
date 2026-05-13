//! Typed index types and arena allocators.

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

// No derive macro – we implement everything manually to avoid requiring T: Copy/Eq
pub struct Idx<T> {
    raw: u32,
    _marker: PhantomData<T>,
}

impl<T> Clone for Idx<T> {
    fn clone(&self) -> Self { *self }
}
impl<T> Copy for Idx<T> {}
impl<T> PartialEq for Idx<T> {
    fn eq(&self, other: &Self) -> bool { self.raw == other.raw }
}
impl<T> Eq for Idx<T> {}
impl<T> std::hash::Hash for Idx<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.raw.hash(state); }
}
impl<T> PartialOrd for Idx<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.raw.partial_cmp(&other.raw)
    }
}
impl<T> Ord for Idx<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}
impl<T> fmt::Debug for Idx<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Idx({})", self.raw)
    }
}

impl<T> Idx<T> {
    #[inline]
    pub fn from_raw(raw: u32) -> Self {
        Self { raw, _marker: PhantomData }
    }
    #[inline]
    pub fn to_raw(self) -> u32 { self.raw }
    #[inline]
    pub fn index(self) -> usize { self.raw as usize }
}

pub trait IdxLike: Copy + Eq + fmt::Debug + 'static {
    fn from_raw(raw: u32) -> Self;
    fn to_raw(self) -> u32;
    fn index(self) -> usize { self.to_raw() as usize }
}

// T must be 'static to satisfy the trait bound, but Idx<T> itself is 'static if T: 'static
impl<T: 'static> IdxLike for Idx<T> {
    fn from_raw(raw: u32) -> Self { Idx::from_raw(raw) }
    fn to_raw(self) -> u32 { self.to_raw() }
}

#[macro_export]
macro_rules! define_idx {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u32);

        impl $name {
            #[inline]
            pub fn from_raw(raw: u32) -> Self { Self(raw) }
            #[inline]
            pub fn to_raw(self) -> u32 { self.0 }
            #[inline]
            pub fn index(self) -> usize { self.0 as usize }
        }

        impl $crate::arena::IdxLike for $name {
            fn from_raw(raw: u32) -> Self { Self(raw) }
            fn to_raw(self) -> u32 { self.0 }
        }
    };
}

#[derive(Clone, Debug)]
pub struct IndexVec<I: IdxLike, T> {
    raw: Vec<T>,
    _marker: PhantomData<I>,
}

impl<I: IdxLike, T> IndexVec<I, T> {
    pub fn new() -> Self { Self { raw: Vec::new(), _marker: PhantomData } }
    pub fn with_capacity(cap: usize) -> Self { Self { raw: Vec::with_capacity(cap), _marker: PhantomData } }
    pub fn from_raw(raw: Vec<T>) -> Self { Self { raw, _marker: PhantomData } }
    pub fn push(&mut self, val: T) -> I {
        let idx = I::from_raw(self.raw.len() as u32);
        self.raw.push(val);
        idx
    }
    pub fn reserve(&mut self, additional: usize) { self.raw.reserve(additional); }
    pub fn len(&self) -> usize { self.raw.len() }
    pub fn is_empty(&self) -> bool { self.raw.is_empty() }
    pub fn get(&self, idx: I) -> Option<&T> { self.raw.get(idx.index()) }
    pub fn get_mut(&mut self, idx: I) -> Option<&mut T> { self.raw.get_mut(idx.index()) }
    pub fn iter(&self) -> impl Iterator<Item = &T> { self.raw.iter() }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> { self.raw.iter_mut() }
    pub fn iter_enumerated(&self) -> impl Iterator<Item = (I, &T)> {
        self.raw.iter().enumerate().map(|(i, v)| (I::from_raw(i as u32), v))
    }
    pub fn into_iter_enumerated(self) -> impl Iterator<Item = (I, T)> {
        self.raw.into_iter().enumerate().map(|(i, v)| (I::from_raw(i as u32), v))
    }
    pub fn into_raw(self) -> Vec<T> { self.raw }
    pub fn as_slice(&self) -> &[T] { &self.raw }
    pub fn as_mut_slice(&mut self) -> &mut [T] { &mut self.raw }
    pub fn last(&self) -> Option<&T> { self.raw.last() }
}

impl<I: IdxLike, T> Default for IndexVec<I, T> {
    fn default() -> Self { Self::new() }
}

impl<I: IdxLike, T> Index<I> for IndexVec<I, T> {
    type Output = T;
    fn index(&self, idx: I) -> &T { &self.raw[idx.index()] }
}

impl<I: IdxLike, T> IndexMut<I> for IndexVec<I, T> {
    fn index_mut(&mut self, idx: I) -> &mut T { &mut self.raw[idx.index()] }
}
