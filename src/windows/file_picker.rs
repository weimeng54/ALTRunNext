// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! File selection and drag-and-drop adapters shared by native windows.
use std::{mem::zeroed, path::PathBuf};
use windows_sys::Win32::{Foundation::HWND, UI::Controls::Dialogs::*};

pub(crate) fn pick_path(hwnd: HWND, save: bool, folder: bool) -> Option<PathBuf> {
    pick_path_filtered(hwnd, save, folder, None)
}

pub(crate) fn pick_path_filtered(
    hwnd: HWND,
    save: bool,
    folder: bool,
    filter: Option<&[u16]>,
) -> Option<PathBuf> {
    unsafe {
        if folder {
            use windows_sys::Win32::{System::Com::*, UI::Shell::*};
            let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
            let mut info: BROWSEINFOW = zeroed();
            info.hwndOwner = hwnd;
            info.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
            let pidl = SHBrowseForFolderW(&info);
            let mut b = vec![0u16; 32768];
            let ok = !pidl.is_null() && SHGetPathFromIDListW(pidl, b.as_mut_ptr()) != 0;
            if !pidl.is_null() {
                CoTaskMemFree(pidl as _);
            }
            if hr >= 0 {
                CoUninitialize();
            }
            if !ok {
                return None;
            }
            return Some(PathBuf::from(String::from_utf16_lossy(
                &b[..b.iter().position(|v| *v == 0).unwrap_or(0)],
            )));
        }
        pick_file(hwnd, save, filter, None)
    }
}

pub(crate) fn save_file(
    owner: HWND,
    filter: &[u16],
    filename: &str,
    extension: &str,
) -> Option<PathBuf> {
    pick_file(owner, true, Some(filter), Some((filename, extension)))
}

fn pick_file(
    owner: HWND,
    save: bool,
    filter: Option<&[u16]>,
    suggested: Option<(&str, &str)>,
) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;

    unsafe {
        let mut path = vec![0u16; 32768];
        let default_extension = suggested.map(|(name, extension)| {
            let name = crate::ui::wide(name);
            path[..name.len()].copy_from_slice(&name);
            crate::ui::wide(extension)
        });
        let mut dialog = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: owner,
            lpstrFile: path.as_mut_ptr(),
            nMaxFile: path.len() as u32,
            lpstrFilter: filter.map_or(std::ptr::null(), |filter| filter.as_ptr()),
            lpstrDefExt: default_extension
                .as_ref()
                .map_or(std::ptr::null(), |ext| ext.as_ptr()),
            Flags: OFN_NOCHANGEDIR
                | OFN_EXPLORER
                | if save {
                    OFN_OVERWRITEPROMPT
                } else {
                    OFN_FILEMUSTEXIST
                },
            ..zeroed()
        };
        let accepted = if save {
            GetSaveFileNameW(&mut dialog)
        } else {
            GetOpenFileNameW(&mut dialog)
        };
        (accepted != 0).then(|| {
            let end = path
                .iter()
                .position(|&unit| unit == 0)
                .unwrap_or(path.len());
            PathBuf::from(std::ffi::OsString::from_wide(&path[..end]))
        })
    }
}
pub(crate) fn drop_paths(handle: windows_sys::Win32::UI::Shell::HDROP) -> Vec<PathBuf> {
    unsafe {
        use windows_sys::Win32::UI::Shell::*;
        let count = DragQueryFileW(handle, u32::MAX, std::ptr::null_mut(), 0).min(1000);
        let mut paths = Vec::new();
        for i in 0..count {
            let n = DragQueryFileW(handle, i, std::ptr::null_mut(), 0);
            if n > 32767 {
                continue;
            }
            let mut b = vec![0u16; n as usize + 1];
            DragQueryFileW(handle, i, b.as_mut_ptr(), b.len() as u32);
            paths.push(PathBuf::from(String::from_utf16_lossy(&b[..n as usize])));
        }
        DragFinish(handle);
        paths
    }
}
