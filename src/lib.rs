#![no_std]
#![feature(allocator_api)]
#![feature(doc_auto_cfg)]
//! # Dodgems - A simple bump allocator library
//!
//! This crate provides a fast, single-threaded [bump allocator](BumpCar) for use in performance
//! sensitive contexts.
//!
//! **⚠️ It is not a general purpose allocator: you need to have another
//! (supposedly slower) allocator to back it up.** By default, it is the global allocator.
//!
//! It can be used for quick and dirty allocation in a loop, where you know memory
//! can be reclaimed all at once at the end.
//!
//! ## Example
//! ```rust
//! #![feature(allocator_api)]
//! # #[cfg(feature = "alloc")]
//! # {
//! use dodgems::BumpCar;
//!
//! let mut bumpcar = BumpCar::new(1024).unwrap(); // 1kB capacity
//!
//! for i in 0..100 {
//!     let mut v = Vec::new_in(&bumpcar); // allocate with the allocator api
//!     v.push(42);
//!     // small fast allocations in hot loop
//!     drop(v);
//!     bumpcar.reset(); // reset the capacity once every allocation has been dropped
//! }
//!
//! drop(bumpcar)
//! # }
//! ```
//!
//! Until the `allocator_api` is stable, this crate requires nightly.
//!
//! ## Features
//! The (default) `alloc` feature controls wether the `alloc` standard crate is used.
//! If you want to use a different allocator and/or do not have a global allocator available,
//! you can disable it.

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::alloc::Global;
use core::alloc::{AllocError, Allocator, Layout};
use core::{cell::Cell, mem::size_of, ptr::NonNull};

/// Returns the next multiple of `align` greater than `size`
///
/// # SAFETY
/// `size + align` must not overflow, and `align` must be a power of two.
unsafe fn next_multiple(size: usize, align: usize) -> usize {
    let am = align - 1;
    (size + am) ^ am
}

/// Fast bump allocator.
///
/// Allocations are made by incrementing an offset, and are tied to the lifetime of a reference
/// to the allocation until the `BumpCar` is dropped or reset.
///
/// # Example
/// ```rust
/// #![feature(allocator_api)]
/// # extern crate alloc;
/// # use alloc::alloc::Global;
/// use dodgems::BumpCar;
///
/// let mut bumpcar = BumpCar::new_in(256, Global).unwrap();
/// let my_box = Box::new_in([1, 2, 3], &bumpcar);
///
/// // drop(bumpcar) <- doesn't compile
/// drop(my_box);
/// drop(bumpcar);
/// ```
pub struct BumpCar<
    #[cfg(feature = "alloc")] A: Allocator = Global,
    #[cfg(not(feature = "alloc"))] A: Allocator,
> {
    pointer: NonNull<[u8]>,
    position: Cell<usize>,
    allocator: A,
}

impl<A: Allocator> BumpCar<A> {
    /// Allocates a new [`BumpCar`] in the given allocator.
    ///
    /// # Errors
    /// This function returns an error if the capacity (or the nearest pointer-aligned multiple)
    /// is greater than [`isize::MAX`], or if the underlying allocator returns an error.
    #[allow(clippy::missing_panics_doc)]
    pub fn new_in(capacity: usize, allocator: A) -> Result<Self, AllocError> {
        // SAFETY: capacity must be <= isize::MAX for next_multiple to be evaluated,
        // and size_of::<usize>() is a power of two.
        if capacity > isize::MAX as _
            || unsafe { next_multiple(capacity, size_of::<usize>()) } > isize::MAX as _
        {
            return Err(AllocError);
        }

        let pointer = allocator
            .allocate(Layout::from_size_align(capacity, core::mem::size_of::<usize>()).unwrap())?;

        Ok(Self {
            pointer,
            position: Cell::new(0),
            allocator,
        })
    }

    /// Returns the capacity of the [`BumpCar`].
    pub fn capacity(&self) -> usize {
        self.pointer.len()
    }

    /// Returns the remaining capacity of the [`BumpCar`].
    ///
    /// This does not guarantee that an allocation of this size will succeed:
    /// the [`BumpCar`] will waste space if a new allocation with a greater alignment
    /// than the last one is required.
    ///
    /// If you need to check for the validity of an allocation in a more precise way,
    /// use [`BumpCar::can_allocate`].
    pub fn remaining_capacity(&self) -> usize {
        self.capacity() - self.position.get()
    }

    /// Checks wether the allocator has enough remaining capacity for the
    /// allocation specified in `layout`.
    pub fn can_allocate(&self, layout: Layout) -> bool {
        // SAFETY: layout.align() is guaranteed to be a power of two,
        // and self.position() <= pointer.len() <= isize::MAX, so the operation cannot overflow.
        let closest_align = unsafe { next_multiple(self.position.get(), layout.align()) };

        let Some(new_pos) = closest_align.checked_add(layout.size()) else {
            return false;
        };

        new_pos <= self.pointer.len()
    }

    /// Resets the [`BumpCar`]'s remaining capacity to its initial capacity.
    ///
    /// This requires a mutable reference, so that any previous allocations made with &self
    /// are invalidated by the borrow checker.
    pub fn reset(&mut self) {
        self.position.set(0);
    }
}

#[cfg(feature = "alloc")]
impl BumpCar {
    /// Allocates a [`BumpCar`] with the Global allocator.
    ///
    /// # Errors
    /// This function returns an error if the capacity (or its nearest pointer-aligned multiple)
    /// is greater than [`isize::MAX`], or if the global returns an error.
    pub fn new(capacity: usize) -> Result<Self, AllocError> {
        Self::new_in(capacity, Global)
    }
}

impl<A: Allocator> Drop for BumpCar<A> {
    /// Deallocates the [`BumpCar`]'s buffer.
    fn drop(&mut self) {
        let ptr = self.pointer.cast::<u8>();
        // SAFETY: ptr is always allocated with self.allocator
        // and the alignement has been validated at construction of the BumpCar
        unsafe {
            self.allocator.deallocate(
                ptr,
                Layout::from_size_align_unchecked(self.pointer.len(), size_of::<usize>()),
            );
        }
    }
}

unsafe impl<'a, A: Allocator> Allocator for &'a BumpCar<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: layout.align() is guaranteed to be a power of two,
        // and self.position() <= pointer.len() <= isize::MAX, so the operation cannot overflow.
        let closest_align = unsafe { next_multiple(self.position.get(), layout.align()) };

        let new_pos = closest_align.checked_add(layout.size()).ok_or(AllocError)?;
        if new_pos > self.pointer.len() {
            return Err(AllocError);
        }

        // SAFETY: closest_align + layout.size() <= pointer.len() <= isize::MAX
        let ptr = unsafe { self.pointer.as_ptr().cast::<u8>().add(closest_align) };
        self.position.set(new_pos);
        Ok(NonNull::slice_from_raw_parts(
            // SAFETY: pointer is non null, and closest_align + layout.size() <= pointer.len(),
            // so ptr = pointer + closest_align is non null.
            unsafe { NonNull::new_unchecked(ptr) },
            layout.size(),
        ))
    }

    /// The [`BumpCar`] does not perform deallocation unless it's reset or dropped.
    unsafe fn deallocate(&self, _: NonNull<u8>, _: Layout) {}

    /// Shrinks an allocated region.
    ///
    /// The [`BumpCar`] allocator has the extra requirement
    /// that the old layout's alignment MUST be bigger than the new one.
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() <= old_layout.size(),
            "`new_layout.size()` must be smaller than or equal to `old_layout.size()`"
        );
        if old_layout.align() < new_layout.align() {
            return Err(AllocError);
        }

        Ok(NonNull::slice_from_raw_parts(ptr, new_layout.size()))
    }
}
