use std::{ffi::c_void, os::windows::ffi::OsStrExt, path::Path, ptr::null_mut};
use windows_sys::{
    Win32::{
        Foundation::RPC_E_CHANGED_MODE, Storage::FileSystem::WIN32_FIND_DATAW, System::Com::*,
    },
    core::{GUID, HRESULT},
};

const SHELL_LINK: GUID = GUID::from_u128(0x00021401_0000_0000_c000_000000000046);
const SHELL_LINK_W: GUID = GUID::from_u128(0x000214f9_0000_0000_c000_000000000046);
const PERSIST_FILE: GUID = GUID::from_u128(0x0000010b_0000_0000_c000_000000000046);

#[repr(C)]
struct Unknown {
    query: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}

type ReadString = unsafe extern "system" fn(*mut c_void, *mut u16, i32) -> HRESULT;

#[repr(C)]
struct ShellLink {
    unknown: Unknown,
    get_path: unsafe extern "system" fn(
        *mut c_void,
        *mut u16,
        i32,
        *mut WIN32_FIND_DATAW,
        u32,
    ) -> HRESULT,
    get_id_list: usize,
    set_id_list: usize,
    get_description: usize,
    set_description: usize,
    get_working_directory: ReadString,
    set_working_directory: usize,
    get_arguments: ReadString,
}

#[repr(C)]
struct PersistFile {
    unknown: Unknown,
    get_class_id: usize,
    is_dirty: usize,
    load: unsafe extern "system" fn(*mut c_void, *const u16, u32) -> HRESULT,
}

struct Interface(*mut c_void);
impl Interface {
    fn vtable<T>(&self) -> &T {
        // These interfaces are obtained from COM for the matching IID above.
        unsafe { &**self.0.cast::<*const T>() }
    }
}
impl Drop for Interface {
    fn drop(&mut self) {
        unsafe { (self.vtable::<Unknown>().release)(self.0) };
    }
}

struct Apartment(bool);
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

pub struct Target {
    pub path: String,
    pub arguments: String,
    pub working_directory: String,
}

pub fn read(path: &Path) -> Option<Target> {
    unsafe {
        let hr = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED as u32);
        if hr < 0 && hr != RPC_E_CHANGED_MODE {
            return None;
        }
        let _apartment = Apartment(hr >= 0);
        let mut raw = null_mut();
        if CoCreateInstance(
            &SHELL_LINK,
            null_mut(),
            CLSCTX_INPROC_SERVER,
            &SHELL_LINK_W,
            &mut raw,
        ) < 0
            || raw.is_null()
        {
            return None;
        }
        let link = Interface(raw);
        let shell = link.vtable::<ShellLink>();
        let mut raw_file = null_mut();
        if (shell.unknown.query)(link.0, &PERSIST_FILE, &mut raw_file) < 0 || raw_file.is_null() {
            return None;
        }
        let file = Interface(raw_file);
        let path = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        if (file.vtable::<PersistFile>().load)(file.0, path.as_ptr(), STGM_READ) < 0 {
            return None;
        }
        let mut buffer = vec![0; 32768];
        let mut read = |get: &mut dyn FnMut(*mut u16, i32) -> HRESULT| {
            buffer.fill(0);
            if get(buffer.as_mut_ptr(), buffer.len() as i32) < 0 {
                return None;
            }
            let end = buffer.iter().position(|&unit| unit == 0)?;
            String::from_utf16(&buffer[..end]).ok()
        };
        // Read the stored target without interactive resolution or running the link.
        let path = read(&mut |out, len| (shell.get_path)(link.0, out, len, null_mut(), 0))?;
        let arguments = read(&mut |out, len| (shell.get_arguments)(link.0, out, len))?;
        let working_directory =
            read(&mut |out, len| (shell.get_working_directory)(link.0, out, len))?;
        Some(Target {
            path,
            arguments,
            working_directory,
        })
    }
}
