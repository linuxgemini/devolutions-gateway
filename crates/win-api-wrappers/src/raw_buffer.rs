use core::fmt;
use core::ptr::NonNull;
use std::alloc::{alloc_zeroed, dealloc, realloc, Layout};

/// The `AllocError` error indicates an allocation failure
/// that may be due to resource exhaustion or to
/// something wrong when combining the given input arguments with this
/// allocator.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct AllocError;

impl std::error::Error for AllocError {}

impl fmt::Display for AllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("memory allocation failed")
    }
}

/// A RAII wrapper around a raw buffer.
///
/// Basically a convenient wrapper around the Rust std global allocator.
/// Useful for DST structs found across the Windows API.
///
/// `ref_cast` and `ref_mut_cast` are used to retrieve a typed reference on the underlying data.
/// This reference lifetime is bounded to the [`RawBuffer`] instance to prevent use-after-free bugs.
pub struct RawBuffer {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl RawBuffer {
    /// Allocates memory as described by the given `layout`, ensuring that the contents are set to zero.
    ///
    /// # Safety
    ///
    /// See [`GlobalAlloc::alloc_zeroed`].
    pub unsafe fn alloc_zeroed(layout: Layout) -> Result<Self, AllocError> {
        let ptr = unsafe { alloc_zeroed(layout) };

        if let Some(ptr) = NonNull::new(ptr) {
            Ok(Self { ptr, layout })
        } else {
            Err(AllocError)
        }
    }

    /// Shrinks or grows the memory to the given `new_size` in bytes.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    ///
    /// * `new_size` is greater than zero.
    ///
    /// * `new_size`, when rounded up to the nearest multiple of `layout.align()`,
    ///   does not overflow `isize` (i.e., the rounded value must be less than or
    ///   equal to `isize::MAX`).
    pub unsafe fn realloc(&mut self, new_size: usize) -> Result<(), AllocError> {
        let new_ptr = unsafe { realloc(self.ptr.as_ptr(), self.layout, new_size) };

        if let Some(new_ptr) = NonNull::new(new_ptr) {
            self.ptr = new_ptr;

            // SAFETY: the caller must ensure that the `new_size` does not overflow.
            // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid.
            let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, self.layout.align()) };

            self.layout = new_layout;

            Ok(())
        } else {
            Err(AllocError)
        }
    }

    /// Casts the underlying raw buffer and returns a reference to it.
    ///
    /// # Safety
    ///
    /// The underlying buffer must hold a valid, initialized `T`.
    pub unsafe fn as_ref_cast<T>(&self) -> &T {
        unsafe { self.ptr.cast::<T>().as_ref() }
    }

    /// Casts the underlying raw buffer and returns a mutable reference to it.
    ///
    /// # Safety
    ///
    /// The underlying buffer must hold a valid, initialized `T`.
    pub unsafe fn as_mut_cast<T>(&mut self) -> &mut T {
        unsafe { self.ptr.cast::<T>().as_mut() }
    }

    pub const fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr().cast_const()
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Obtains an [`InitedBuffer`] guaranteed to contain a T.
    ///
    /// # Safety
    ///
    /// This raw buffer must hold a valid, properly initialized T.
    pub unsafe fn assume_init<T>(self) -> InitedBuffer<T> {
        InitedBuffer {
            inner: self, // The caller guarantees this raw buffer is holding a valid, properly initialized T.
            _marker: core::marker::PhantomData,
        }
    }
}

impl Drop for RawBuffer {
    fn drop(&mut self) {
        // SAFETY:
        // - ptr is a block of memory currently allocated via the global allocator and,
        // - layout is the same layout that was used to allocate that block of memory.
        unsafe { dealloc(self.ptr.as_ptr(), self.layout) };
    }
}

/// A buffer that is guaranteed to hold a properly initialized T.
///
/// If you use `realloc`, you should ensure that the `T` value is still valid
/// before any call to [`InitedBuffer::as_ref`] or [`InitedBuffer::as_ref_mut`].
pub struct InitedBuffer<T> {
    /// INVARIANT: This raw buffer holds a properly initialized T.
    inner: RawBuffer,
    _marker: core::marker::PhantomData<*mut T>,
}

impl<T> AsRef<RawBuffer> for InitedBuffer<T> {
    fn as_ref(&self) -> &RawBuffer {
        &self.inner
    }
}

impl<T> std::ops::Deref for InitedBuffer<T> {
    type Target = RawBuffer;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> std::ops::DerefMut for InitedBuffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T> InitedBuffer<T> {
    pub fn as_ref(&self) -> &T {
        // SAFETY: Per invariants, the inner RawBuffer holds a properly initialized T.
        unsafe { self.inner.as_ref_cast::<T>() }
    }

    pub fn as_mut(&mut self) -> &mut T {
        // SAFETY: Per invariants, the inner RawBuffer holds a properly initialized T.
        unsafe { self.inner.as_mut_cast::<T>() }
    }
}
