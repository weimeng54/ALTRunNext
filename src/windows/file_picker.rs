// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! File selection and drag-and-drop adapters shared by native windows.
use std::{mem::zeroed, path::PathBuf};
use windows_sys::Win32::{Foundation::HWND, UI::Controls::Dialogs::*};

pub(crate) fn pick_path(hwnd: HWND, folder: bool) -> Option<PathBuf> {
    if !folder {
        return pick_file(hwnd, None, None);
    }
    unsafe {
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
        Some(PathBuf::from(String::from_utf16_lossy(
            &b[..b.iter().position(|v| *v == 0).unwrap_or(0)],
        )))
    }
}

pub(crate) fn open_file(owner: HWND, filter: &[u16]) -> Option<PathBuf> {
    pick_file(owner, Some(filter), None)
}

pub(crate) fn file_types(types: &[(&str, &str)]) -> Vec<u16> {
    let mut filter = Vec::new();
    for &(key, pattern) in types {
        let label = crate::localization::tr(key);
        filter.extend(crate::ui::wide(&format!("{label} ({pattern})")));
        filter.extend(crate::ui::wide(pattern));
    }
    filter.push(0);
    filter
}

#[derive(Clone, Copy)]
pub(crate) enum SaveExtension<'a> {
    Default(&'a str),
    // The caller confirms replacement after the final path has been resolved.
    Required(&'a str),
}

pub(crate) fn save_file(
    owner: HWND,
    filter: &[u16],
    filename: &str,
    extension: SaveExtension<'_>,
) -> Option<PathBuf> {
    let mut path = pick_file(owner, Some(filter), Some((filename, extension)))?;
    if let SaveExtension::Required(ext) = extension
        && !path
            .extension()
            .is_some_and(|actual| actual.eq_ignore_ascii_case(ext))
    {
        path.set_extension(ext);
    }
    Some(path)
}

fn pick_file(
    owner: HWND,
    filter: Option<&[u16]>,
    suggested: Option<(&str, SaveExtension<'_>)>,
) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;

    unsafe {
        let mut path = vec![0u16; 32768];
        let (flags, default_extension) = if let Some((name, extension)) = suggested {
            let name = crate::ui::wide(name);
            path[..name.len()].copy_from_slice(&name);
            match extension {
                SaveExtension::Default(ext) => (OFN_OVERWRITEPROMPT, Some(crate::ui::wide(ext))),
                SaveExtension::Required(_) => (OFN_NOTESTFILECREATE, None),
            }
        } else {
            (OFN_FILEMUSTEXIST, None)
        };
        let mut dialog = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: owner,
            lpstrFile: path.as_mut_ptr(),
            nMaxFile: path.len() as u32,
            lpstrFilter: filter.map_or(std::ptr::null(), |filter| filter.as_ptr()),
            lpstrDefExt: default_extension
                .as_ref()
                .map_or(std::ptr::null(), |ext| ext.as_ptr()),
            nFilterIndex: 1,
            Flags: OFN_NOCHANGEDIR | OFN_EXPLORER | flags,
            ..zeroed()
        };
        let accepted = if suggested.is_some() {
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

pub(crate) fn pick_program(owner: HWND) -> Result<Option<String>, String> {
    let mut filter = crate::ui::ui("ui.executable_programs");
    filter.extend(crate::ui::wide("*.exe"));
    filter.push(0);
    let Some(path) = open_file(owner, &filter) else {
        return Ok(None);
    };
    if !path.is_file() || !altrun_probe::management::is_executable(&path) {
        return Err("ui.select_a_valid_exe_application".into());
    }
    altrun_probe::management::absolute_path(&path).map(Some)
}
