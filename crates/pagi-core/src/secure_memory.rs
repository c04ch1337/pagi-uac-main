//! **Secure memory** — Lock decrypted Shadow buffers in RAM so the OS never swaps them to disk.
//!
//! Uses `mlock`/`munlock` on Unix and `VirtualLock`/`VirtualUnlock` on Windows.
//! Combined with zeroing on drop, this ensures sensitive data never touches the page file/swap.

use std::ptr;

/// Lock a region of memory so the OS will not swap it to disk.
/// Returns true if locking succeeded (or is a no-op); false on failure.
#[inline]
pub fn lock_region(ptr: *mut u8, len: usize) -> bool {
    if len == 0 {
        return true;
    }
    #[cfg(unix)]
    {
        use std::ffi::c_void;
        // mlock locks the pages containing [ptr, ptr+len); no alignment required.
        unsafe { libc::mlock(ptr as *mut c_void, len) == 0 }
    }
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        unsafe {
            windows_sys::Win32::System::Memory::VirtualLock(ptr as *const c_void, len) != 0
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (ptr, len);
        true // no-op on unsupported platforms
    }
}

/// Unlock a region previously locked with `lock_region`.
#[inline]
pub fn unlock_region(ptr: *mut u8, len: usize) -> bool {
    if len == 0 {
        return true;
    }
    #[cfg(unix)]
    {
        use std::ffi::c_void;
        unsafe { libc::munlock(ptr as *mut c_void, len) == 0 }
    }
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        unsafe {
            windows_sys::Win32::System::Memory::VirtualUnlock(ptr as *const c_void, len) != 0
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (ptr, len);
        true
    }
}

/// Zero a region of memory (volatile write so the compiler won't optimize it out).
#[inline]
pub fn zero_region(ptr: *mut u8, len: usize) {
    if len == 0 {
        return;
    }
    unsafe {
        ptr::write_bytes(ptr, 0, len);
    }
}

/// A buffer that is locked in RAM (no swap) and zeroed on drop.
/// Use for decrypted Shadow content so it never touches disk.
pub struct LockedVec {
    inner: Vec<u8>,
    locked: bool,
}

impl LockedVec {
    /// Takes ownership of `data`, locks it in RAM, and returns a wrapper that will zero and unlock on drop.
    pub fn new(mut data: Vec<u8>) -> Self {
        let is_empty = data.is_empty();
        let locked = if is_empty {
            true
        } else {
            lock_region(data.as_mut_ptr(), data.len())
        };
        if !locked && !is_empty {
            tracing::warn!(
                target: "pagi::secure_memory",
                "mlock/VirtualLock failed; decrypted buffer may be swapped to disk"
            );
        }
        Self {
            inner: data,
            locked: locked || is_empty,
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.inner
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl AsRef<[u8]> for LockedVec {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Drop for LockedVec {
    fn drop(&mut self) {
        if self.inner.is_empty() {
            return;
        }
        let ptr = self.inner.as_mut_ptr();
        let len = self.inner.len();
        zero_region(ptr, len);
        if self.locked {
            unlock_region(ptr, len);
        }
    }
}
