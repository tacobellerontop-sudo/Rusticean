//! Named mutexes, so two bootstrapper processes never upgrade Roblox at the same time.

use std::io;

/// Held while the named mutex is owned; releases it on drop.
pub struct NamedMutexGuard {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

// SAFETY: a mutex handle is just a kernel object reference; we only release it on drop.
#[cfg(windows)]
unsafe impl Send for NamedMutexGuard {}

/// Block until the session-wide mutex `name` is ours.
#[cfg(windows)]
pub fn acquire(name: &str) -> io::Result<NamedMutexGuard> {
    use windows_sys::Win32::Foundation::{WAIT_ABANDONED, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{CreateMutexW, INFINITE, WaitForSingleObject};

    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `wide` is a valid NUL-terminated UTF-16 string for the duration of the call.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: `handle` is a valid mutex handle we just created.
    match unsafe { WaitForSingleObject(handle, INFINITE) } {
        // an abandoned mutex still becomes ours; the previous owner crashed
        WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(NamedMutexGuard { handle }),
        _ => {
            let err = io::Error::last_os_error();
            // SAFETY: closing the handle we own.
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
            Err(err)
        }
    }
}

/// Take the mutex `name` if nobody holds it; `None` if another process does.
#[cfg(windows)]
pub fn try_acquire(name: &str) -> io::Result<Option<NamedMutexGuard>> {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};

    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `wide` is a valid NUL-terminated UTF-16 string for the duration of the call.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `handle` is a valid mutex handle we just created.
    match unsafe { WaitForSingleObject(handle, 0) } {
        WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Some(NamedMutexGuard { handle })),
        _ => {
            // SAFETY: closing the handle we own.
            unsafe { CloseHandle(handle) };
            Ok(None)
        }
    }
}

#[cfg(not(windows))]
pub fn try_acquire(_name: &str) -> io::Result<Option<NamedMutexGuard>> {
    Ok(Some(NamedMutexGuard {}))
}

/// Roblox's single-instance mutex. A new Roblox closes the old one only if it gets to
/// own this, so holding it first lets several clients run side by side.
pub const ROBLOX_SINGLETON: &str = "ROBLOX_singletonMutex";

/// Keep [`ROBLOX_SINGLETON`] in existence (and owned, if nobody else has it) until the
/// guard is dropped. Never waits, even if a running Roblox owns it.
#[cfg(windows)]
pub fn hold_roblox_singleton() -> io::Result<NamedMutexGuard> {
    use windows_sys::Win32::System::Threading::CreateMutexW;

    let wide: Vec<u16> = ROBLOX_SINGLETON
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `wide` is a valid NUL-terminated UTF-16 string for the duration of the call.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 1, wide.as_ptr()) };
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(NamedMutexGuard { handle })
}

#[cfg(not(windows))]
pub fn hold_roblox_singleton() -> io::Result<NamedMutexGuard> {
    Ok(NamedMutexGuard {})
}

#[cfg(not(windows))]
pub fn acquire(_name: &str) -> io::Result<NamedMutexGuard> {
    Ok(NamedMutexGuard {})
}

#[cfg(windows)]
impl Drop for NamedMutexGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::ReleaseMutex;
        // SAFETY: we own the mutex and the handle.
        unsafe {
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

/// Whether a Roblox Player is open, judged by the singleton mutex it creates.
/// (It can linger a few seconds after the window closes.)
#[cfg(windows)]
pub fn is_roblox_running() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};

    let name: Vec<u16> = ROBLOX_SINGLETON
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `name` is NUL-terminated; a non-null handle is closed straight away.
    unsafe {
        let handle = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
        if handle.is_null() {
            false
        } else {
            CloseHandle(handle);
            true
        }
    }
}

#[cfg(not(windows))]
pub fn is_roblox_running() -> bool {
    false
}
