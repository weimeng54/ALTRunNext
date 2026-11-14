// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::ui::wide;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::HWND;

pub(crate) fn clipboard_text() -> Option<String> {
    use windows_sys::Win32::System::{DataExchange::*, Memory::*};
    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return None;
        }
        let handle = GetClipboardData(13);
        let result = if handle.is_null() {
            None
        } else {
            let pointer = GlobalLock(handle) as *const u16;
            if pointer.is_null() {
                None
            } else {
                let length = GlobalSize(handle) / 2;
                let result = if length > 1024 * 1024 {
                    None
                } else {
                    let slice = std::slice::from_raw_parts(pointer, length);
                    String::from_utf16(slice.split(|v| *v == 0).next().unwrap_or_default()).ok()
                };
                GlobalUnlock(handle);
                result
            }
        };
        CloseClipboard();
        result
    }
}
pub(crate) fn copy_text(owner: HWND, value: &str) -> Result<(), String> {
    use windows_sys::Win32::System::{DataExchange::*, Memory::*};
    let bytes = wide(value);
    unsafe {
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len() * 2);
        if handle.is_null() {
            return Err("ui.could_not_allocate_clipboard_memory".into());
        }
        let pointer = GlobalLock(handle) as *mut u16;
        if pointer.is_null() {
            GlobalFree(handle);
            return Err("ui.could_not_access_clipboard_memory".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
        GlobalUnlock(handle);
        if OpenClipboard(owner) == 0 {
            GlobalFree(handle);
            return Err("ui.another_app_is_using_the_clipboard".into());
        }
        EmptyClipboard();
        let success = !SetClipboardData(13, handle).is_null();
        CloseClipboard();
        if !success {
            GlobalFree(handle);
            return Err("ui.could_not_write_to_the_clipboard".into());
        }
    }
    Ok(())
}
