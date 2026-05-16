//! Glyim standard library – dynamic allocation and collections.
//!
//! This crate provides:
//! - `alloc::alloc` global allocator API (`GlobalAlloc`, `Layout`)
//! - `Vec<T>` – growable array
//! - `String` – owned, UTF-8 encoded string
//! - `Box<T>` – heap‑allocated owned pointer
//! - `Rc<T>`, `Arc<T>` – reference‑counted pointers
//! - `RawVec<T>` – low‑level buffer management

pub mod alloc;
pub mod boxed;
pub mod raw_vec;
pub mod rc;
pub mod string;
pub mod vec;

pub use alloc::GlobalAlloc;
pub use boxed::Box;
pub use raw_vec::RawVec;
pub use rc::Rc;
pub use string::String;
pub use vec::Vec;

#[cfg(test)]
mod tests;
