use crate::native::wide;
use std::{
    path::{Path, PathBuf},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::*,
    System::{DataExchange::COPYDATASTRUCT, Threading::*},
    UI::WindowsAndMessaging::*,
};
const PROPERTY: &str = "ALTRun.Modern.Dataset";
const MAGIC: usize = 0x4152544d;
pub struct Instance {
    handle: HANDLE,
    pub identity: usize,
}
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
pub fn identity(path: &Path) -> usize {
    let mut hash = 0xcbf29ce484222325u64;
    for b in path.to_string_lossy().to_lowercase().bytes() {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
    }
    hash as usize | 1
}
pub fn acquire(path: &Path, paths: &[PathBuf]) -> Result<Option<Instance>, String> {
    unsafe {
        let identity = identity(path);
        let handle = CreateMutexW(
            std::ptr::null(),
            0,
            wide(&format!("Local\\ALTRunModern.{identity:016x}")).as_ptr(),
        );
        if handle.is_null() {
            return Err("ui.could_not_create_the_instance_lock".into());
        }
        if GetLastError() != ERROR_ALREADY_EXISTS {
            return Ok(Some(Instance { handle, identity }));
        }
        CloseHandle(handle);
        struct Search {
            identity: usize,
            window: HWND,
        }
        unsafe extern "system" fn callback(hwnd: HWND, value: LPARAM) -> i32 {
            let search = unsafe { &mut *(value as *mut Search) };
            if unsafe { GetPropW(hwnd, wide(PROPERTY).as_ptr()) } as usize == search.identity {
                search.window = hwnd;
                0
            } else {
                1
            }
        }
        let mut search = Search {
            identity,
            window: null_mut(),
        };
        for _ in 0..40 {
            EnumWindows(Some(callback), &mut search as *mut _ as isize);
            if !search.window.is_null() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        if search.window.is_null() {
            return Err("ui.altrunnext_is_already_starting_try_again_shortly".into());
        }
        let args = std::env::args().collect::<Vec<_>>();
        let action = if args
            .iter()
            .any(|a| matches!(a.as_str(), "--clean" | "Clean"))
        {
            Some(18009)
        } else if args
            .iter()
            .any(|a| matches!(a.as_str(), "--restart" | "Restart"))
        {
            Some(18008)
        } else {
            None
        };
        if let Some(action) = action {
            PostMessageW(search.window, WM_COMMAND, action, 0);
            return Ok(None);
        }
        let text = paths
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");
        if text.len() > 30000
            || paths
                .iter()
                .any(|p| p.to_string_lossy().contains(['\0', '\r', '\n']))
        {
            return Err("ui.too_many_paths_were_supplied_or_a".into());
        }
        let bytes = wide(&text);
        let data = COPYDATASTRUCT {
            dwData: MAGIC,
            cbData: (bytes.len() * 2) as u32,
            lpData: bytes.as_ptr() as _,
        };
        let mut result = 0;
        AllowSetForegroundWindow(ASFW_ANY);
        if SendMessageTimeoutW(
            search.window,
            WM_COPYDATA,
            0,
            &data as *const _ as isize,
            SMTO_ABORTIFHUNG,
            3000,
            &mut result,
        ) == 0
            || result == 0
        {
            return Err("ui.the_running_instance_cannot_receive_requests_right".into());
        }
        Ok(None)
    }
}
pub fn publish(hwnd: HWND, identity: usize) {
    unsafe {
        SetPropW(hwnd, wide(PROPERTY).as_ptr(), identity as HANDLE);
    }
}
pub fn decode(l: LPARAM) -> Result<Vec<PathBuf>, String> {
    if l == 0 {
        return Err("ui.missing_message".into());
    }
    let data = unsafe { &*(l as *const COPYDATASTRUCT) };
    if data.dwData != MAGIC
        || data.cbData < 2
        || data.cbData > 65536
        || data.cbData % 2 != 0
        || data.lpData.is_null()
    {
        return Err("ui.invalid_path_message".into());
    }
    let bytes =
        unsafe { std::slice::from_raw_parts(data.lpData as *const u8, data.cbData as usize) };
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_ne_bytes(*p))
        .collect::<Vec<_>>();
    if units.last() != Some(&0) {
        return Err("ui.invalid_path_message".into());
    }
    let text =
        String::from_utf16(&units[..units.len() - 1]).map_err(|_| "ui.invalid_path_encoding")?;
    if text.contains(['\0', '\r']) {
        return Err("ui.invalid_path_character".into());
    }
    Ok(text
        .lines()
        .filter(|s| !s.is_empty())
        .take(1000)
        .map(PathBuf::from)
        .collect())
}
