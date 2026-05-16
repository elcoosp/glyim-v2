//! Global allocator API and memory layout for the Glyim alloc library.

/// A `Layout` describes a particular block of memory.
struct Layout {
    size: usize,
    align: usize,
}

impl Layout {
    /// Creates a new `Layout` from the given `size` and `align`.
    fn from_size_align(size: usize, align: usize) -> Result<Layout, LayoutError> {
        if !align.is_power_of_two() {
            return Result::Err(LayoutError::InvalidAlignment(align));
        }
        if size > usize::MAX - (align - 1) {
            return Result::Err(LayoutError::SizeOverflow);
        }
        Result::Ok(Layout { size, align })
    }

    /// The minimum size in bytes for a memory block of this layout.
    fn size(&self) -> usize {
        self.size
    }

    /// The minimum alignment for a memory block of this layout.
    fn align(&self) -> usize {
        self.align
    }
}

/// Error returned by `Layout::from_size_align` when parameters are invalid.
enum LayoutError {
    /// The alignment was not a power of two.
    InvalidAlignment(usize),
    /// The requested size overflows the address space.
    SizeOverflow,
}

/// Trait for custom global allocators.
trait GlobalAlloc {
    /// Allocate memory as described by `layout`.
    fn alloc(&self, layout: Layout) -> *mut u8;

    /// Deallocate the memory referenced by `ptr`.
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout);
}
