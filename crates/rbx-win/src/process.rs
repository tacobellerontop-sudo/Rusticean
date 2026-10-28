//! Finding Roblox processes from a process snapshot, which lists names and ids without
//! ever opening a handle to the game.

pub const PLAYER_EXE: &str = "RobloxPlayerBeta.exe";

/// Ids of every running Roblox Player.
#[cfg(windows)]
pub fn roblox_pids() -> Vec<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let mut pids = Vec::new();
    // SAFETY: the snapshot handle is checked and closed; `entry` is sized as the API needs.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return pids;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry) != 0;
        while ok {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            if name.eq_ignore_ascii_case(PLAYER_EXE) {
                pids.push(entry.th32ProcessID);
            }
            ok = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    pids
}

#[cfg(not(windows))]
pub fn roblox_pids() -> Vec<u32> {
    Vec::new()
}

/// Visible top-level windows owned by `pids`.
#[cfg(windows)]
pub fn windows_of(pids: &[u32]) -> Vec<windows_sys::Win32::Foundation::HWND> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsWindowVisible,
    };

    struct Search<'a> {
        pids: &'a [u32],
        found: Vec<HWND>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> i32 {
        // SAFETY: `lparam` is the `Search` passed to EnumWindows below, alive for the call.
        let search = unsafe { &mut *(lparam as *mut Search) };
        let mut pid = 0;
        // SAFETY: `hwnd` comes from EnumWindows.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            if search.pids.contains(&pid)
                && IsWindowVisible(hwnd) != 0
                && GetWindow(hwnd, GW_OWNER).is_null()
            {
                search.found.push(hwnd);
            }
        }
        1
    }

    let mut search = Search {
        pids,
        found: Vec::new(),
    };
    // SAFETY: `search` outlives the EnumWindows call that uses it.
    unsafe { EnumWindows(Some(visit), &mut search as *mut Search as LPARAM) };
    search.found
}

/// Ask every window of `pids` to close, like clicking its X. Roblox shuts down normally;
/// no process handle is opened.
#[cfg(windows)]
pub fn close_windows(pids: &[u32]) -> usize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let windows = windows_of(pids);
    for &hwnd in &windows {
        // SAFETY: the handle comes from EnumWindows; posting to a closed window just fails.
        unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
    }
    windows.len()
}

#[cfg(not(windows))]
pub fn close_windows(_pids: &[u32]) -> usize {
    0
}
