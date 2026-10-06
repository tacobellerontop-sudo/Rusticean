//! Desktop notifications through a temporary notification-area icon, the way Bloxstrap
//! shows "Connected to server" balloons. Windows 10 and 11 show them as toasts.

use std::time::Duration;

/// Show `title` and `body` for about `duration`, without blocking the caller.
#[cfg(windows)]
pub fn show(title: &str, body: &str, duration: Duration) {
    let (title, body) = (title.to_owned(), body.to_owned());
    std::thread::spawn(move || {
        // SAFETY: every call gets valid, initialised arguments; the window and icon are
        // created, used and destroyed on this one thread.
        unsafe { show_blocking(&title, &body, duration) }
    });
}

#[cfg(not(windows))]
pub fn show(title: &str, body: &str, _duration: Duration) {
    eprintln!("[notification] {title}: {body}");
}

#[cfg(windows)]
unsafe fn show_blocking(title: &str, body: &str, duration: Duration) {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Shell::{
        NIF_ICON, NIF_INFO, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
        Shell_NotifyIconW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, HWND_MESSAGE, IDI_APPLICATION, LoadIconW,
    };

    fn copy_into(dst: &mut [u16], text: &str) {
        let wide: Vec<u16> = text.encode_utf16().take(dst.len() - 1).collect();
        dst[..wide.len()].copy_from_slice(&wide);
        dst[wide.len()] = 0;
    }

    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return;
        }

        // our exe's icon is resource 1 (see rbx-app/build.rs)
        let mut icon = LoadIconW(instance, 1 as _);
        if icon.is_null() {
            icon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        }

        let mut data: NOTIFYICONDATAW = std::mem::zeroed();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 1;
        data.uFlags = NIF_ICON | NIF_TIP | NIF_INFO;
        data.hIcon = icon;
        data.dwInfoFlags = NIIF_INFO;
        copy_into(&mut data.szTip, "Rusticean");
        copy_into(&mut data.szInfoTitle, title);
        copy_into(&mut data.szInfo, body);

        if Shell_NotifyIconW(NIM_ADD, &data) != 0 {
            std::thread::sleep(duration);
            Shell_NotifyIconW(NIM_DELETE, &data);
        }
        DestroyWindow(hwnd);
    }
}
