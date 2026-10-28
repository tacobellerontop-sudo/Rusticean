//! Anti-AFK: Roblox disconnects players after 20 idle minutes, so every so often we
//! bring its window forward, tap a key, and hand focus back. Input goes through
//! `SendInput` like a real keyboard; nothing touches the game's process.

use std::time::Duration;

/// How long the user must have been away from the keyboard and mouse before we take
/// focus, so we never steal keystrokes mid-typing.
pub const USER_IDLE: Duration = Duration::from_secs(3);

/// Time since the user last pressed a key or moved the mouse anywhere.
#[cfg(windows)]
pub fn user_idle_time() -> Duration {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` is initialised with its size as the API requires.
    unsafe {
        if GetLastInputInfo(&mut info) == 0 {
            return Duration::ZERO;
        }
        Duration::from_millis(GetTickCount().wrapping_sub(info.dwTime) as u64)
    }
}

#[cfg(not(windows))]
pub fn user_idle_time() -> Duration {
    Duration::ZERO
}

/// Whether the window the user is looking at belongs to one of `pids`.
#[cfg(windows)]
pub fn is_foreground(pids: &[u32]) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };
    let mut pid = 0;
    // SAFETY: GetWindowThreadProcessId tolerates a null window.
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid) };
    pid != 0 && pids.contains(&pid)
}

#[cfg(not(windows))]
pub fn is_foreground(_pids: &[u32]) -> bool {
    false
}

/// Tap the space bar in every top-level window owned by `pids`. Returns how many
/// windows were nudged.
#[cfg(windows)]
pub fn nudge(pids: &[u32]) -> usize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, IsIconic, SW_MINIMIZE, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };

    let windows = crate::process::windows_of(pids);
    if windows.is_empty() {
        return 0;
    }
    // SAFETY: every handle comes from EnumWindows and is only passed back to user32,
    // which tolerates windows that have since closed.
    unsafe {
        let previous = GetForegroundWindow();
        for &hwnd in &windows {
            let minimized = IsIconic(hwnd) != 0;
            if minimized {
                ShowWindow(hwnd, SW_RESTORE);
            }
            // a tap of Alt lets a background process take the foreground
            send_key(VK_MENU);
            SetForegroundWindow(hwnd);
            std::thread::sleep(Duration::from_millis(150));
            send_key(VK_SPACE);
            std::thread::sleep(Duration::from_millis(100));
            if minimized {
                ShowWindow(hwnd, SW_MINIMIZE);
            }
        }
        if !previous.is_null() {
            send_key(VK_MENU);
            SetForegroundWindow(previous);
        }
    }
    windows.len()
}

#[cfg(not(windows))]
pub fn nudge(_pids: &[u32]) -> usize {
    0
}

#[cfg(windows)]
const VK_MENU: u16 = 0x12;
#[cfg(windows)]
const VK_SPACE: u16 = 0x20;

#[cfg(windows)]
fn send_key(vk: u16) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    };

    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [key(0), key(KEYEVENTF_KEYUP)];
    // SAFETY: `inputs` is a valid array of INPUT structs of the stated size.
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}
