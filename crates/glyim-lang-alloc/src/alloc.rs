//! Global allocator API and layout.
//! Stub – full implementation in progress.

/// A `Layout` describes a particular block of memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    size: usize,
    align: usize,
}

impl Layout {
    /// Creates a new `Layout` from the given `size` and `align`.
    pub fn from_size_align(size: usize, align: usize) -> Result<Self, LayoutError> {
        if !align.is_power_of_two() {
            return Err(LayoutError::InvalidAlignment(align));
        }
        if size > usize::MAX - (align - 1) {
            return Err(LayoutError::SizeOverflow);
        }
        Ok(Layout { size, align })
    }

    /// The minimum size in bytes for a memory block of this layout.
    pub fn size(&self) -> usize { self.size }

    /// The minimum alignment for a memory block of this layout.
    pub fn align(&self) -> usize { self.align }
}

/// Error returned by `Layout::from_size_align` when parameters are invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    /// The alignment was not a power of two.
    InvalidAlignment(usize),
    /// The requested size overflows the address space.
    SizeOverflow,
}

/// Trait for custom global allocators.
pub trait GlobalAlloc {
    /// Allocate memory as described by `layout`.
    ///
    /// Returns a pointer to the allocated memory, or `null()` on failure.
    fn alloc(&self, layout: Layout) -> *mut u8;

    /// Deallocate the memory referenced by `ptr`.
    ///
    /// # Safety
    /// `ptr` must have been returned by a previous call to `alloc` with the same layout.
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout);
}
/// The global allocator instance.
pub static GLOBAL: GlyimAlloc = GlyimAlloc;

/// Concrete global allocator backed by the Glyim runtime.
pub struct GlyimAlloc;

impl GlobalAlloc for GlyimAlloc {
    fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: calls runtime allocator.
        glyim_runtime::glyim_alloc(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: arguments must match a previous alloc.
        unsafe { glyim_runtime::glyim_dealloc(ptr, layout.size(), layout.align()) }
    }
}

/// Abort on memory allocation failure.
pub fn handle_alloc_error(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout);
}
