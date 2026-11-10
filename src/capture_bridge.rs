//! An owned, versioned boundary to the independent desktop-frame observer.
use crate::desktop_frame::DesktopFrame;
use std::{
    ffi::{CStr, c_void},
    marker::PhantomData,
    mem::{size_of, transmute_copy},
    os::windows::ffi::OsStrExt,
    path::Path,
    ptr::{NonNull, null_mut},
    rc::Rc,
};
use windows_sys::Win32::{Foundation::*, System::LibraryLoader::*};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Region {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}
#[repr(C)]
#[derive(Default, Debug)]
struct FrameInfo {
    size: u32,
    version: u32,
    present_qpc: i64,
    acquire_begin_qpc: i64,
    acquire_end_qpc: i64,
    readback_end_qpc: i64,
    accumulated_frames: u32,
    protected_content: u32,
}
const _: () = assert!(size_of::<FrameInfo>() == 48);
type Open = unsafe extern "C" fn(*const Region, *mut *mut c_void) -> i32;
type NativePoll = unsafe extern "C" fn(*mut c_void, u32, *mut u8, u64, *mut FrameInfo) -> i32;
type Close = unsafe extern "C" fn(*mut c_void);
struct Module(HMODULE);
impl Drop for Module {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.0);
        }
    }
}

pub struct Bridge {
    module: HMODULE,
    open: Open,
    poll: NativePoll,
    close: Close,
    _single_thread: PhantomData<Rc<()>>,
}
pub struct Capture {
    bridge: Rc<Bridge>,
    handle: NonNull<c_void>,
    pixels: Vec<u8>,
}
#[derive(Debug)]
pub enum Poll {
    Timeout,
    PointerOnly,
    Frame(DesktopFrame),
}

unsafe fn symbol<T: Copy>(module: HMODULE, name: &CStr) -> Result<T, String> {
    let raw = unsafe { GetProcAddress(module, name.as_ptr().cast()) }
        .ok_or_else(|| format!("Missing capture ABI symbol {}", name.to_string_lossy()))?;
    if size_of::<T>() != size_of_val(&raw) {
        return Err("Capture ABI symbol size mismatch".into());
    }
    // Only private, explicitly declared C function-pointer types use this helper.
    Ok(unsafe { transmute_copy(&raw) })
}
impl Bridge {
    pub fn load_adjacent() -> Result<Rc<Self>, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        Self::load_exact(
            &exe.parent()
                .ok_or("Missing executable parent")?
                .join("altrun-frame-capture.dll"),
        )
    }
    fn load_exact(path: &Path) -> Result<Rc<Self>, String> {
        if !path.is_absolute() {
            return Err("Capture DLL load path must be absolute".into());
        }
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err("Capture DLL load path contains NUL".into());
        }
        wide.push(0);
        let module = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        };
        if module.is_null() {
            return Err(format!("Capture DLL load failed (Win32 {})", unsafe {
                GetLastError()
            }));
        }
        let guard = Module(module);
        let version: unsafe extern "C" fn() -> u32 =
            unsafe { symbol(module, c"altrun_capture_abi_version")? };
        if unsafe { version() } != 1 {
            return Err("Unsupported capture ABI version".into());
        }
        let open = unsafe { symbol(module, c"altrun_capture_open")? };
        let poll = unsafe { symbol(module, c"altrun_capture_poll")? };
        let close = unsafe { symbol(module, c"altrun_capture_close")? };
        let bridge = Self {
            module,
            open,
            poll,
            close,
            _single_thread: PhantomData,
        };
        std::mem::forget(guard);
        Ok(Rc::new(bridge))
    }
    pub fn open(self: &Rc<Self>, region: Region) -> Result<Capture, String> {
        if region.width <= 0 || region.height <= 0 || region.width > 4096 || region.height > 4096 {
            return Err("Invalid capture region".into());
        }
        let length = region.width as usize * region.height as usize * 4;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(length)
            .map_err(|_| "Cannot allocate capture pixels")?;
        pixels.resize(length, 0);
        let mut handle = null_mut();
        let status = unsafe { (self.open)(&region, &mut handle) };
        if status != 0 {
            if !handle.is_null() {
                unsafe {
                    (self.close)(handle);
                }
            }
            return Err(format!(
                "Capture open failed (HRESULT {:#x})",
                status as u32
            ));
        }
        let handle = NonNull::new(handle).ok_or("Capture ABI returned a null owner")?;
        Ok(Capture {
            bridge: self.clone(),
            handle,
            pixels,
        })
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.module);
        }
    }
}
impl Capture {
    pub fn poll(&mut self, timeout_ms: u32) -> Result<Poll, String> {
        if timeout_ms > 50 {
            return Err("Capture poll timeout must be at most 50ms".into());
        }
        let mut info = FrameInfo {
            size: size_of::<FrameInfo>() as u32,
            version: 1,
            ..FrameInfo::default()
        };
        let status = unsafe {
            (self.bridge.poll)(
                self.handle.as_ptr(),
                timeout_ms,
                self.pixels.as_mut_ptr(),
                self.pixels.len() as u64,
                &mut info,
            )
        };
        if (0..=2).contains(&status) {
            if info.size != size_of::<FrameInfo>() as u32 || info.version != 1 {
                return Err("Capture ABI frame layout mismatch".into());
            }
            if info.protected_content != 0 {
                return Err(format!("Protected capture frame metadata: {info:?}"));
            }
            if status != 0 && (info.present_qpc != 0 || info.accumulated_frames != 0) {
                return Err(format!("Inconsistent no-update frame metadata: {info:?}"));
            }
        }
        match status {
            0 => Ok(Poll::Frame(DesktopFrame {
                present_qpc: info.present_qpc,
                acquire_begin_qpc: info.acquire_begin_qpc,
                acquire_end_qpc: info.acquire_end_qpc,
                readback_end_qpc: info.readback_end_qpc,
                accumulated_frames: info.accumulated_frames,
                protected_content: info.protected_content != 0,
            })),
            1 => Ok(Poll::Timeout),
            2 => Ok(Poll::PointerOnly),
            other => Err(format!(
                "Capture poll failed (HRESULT {:#x}); metadata {info:?}",
                other as u32
            )),
        }
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            (self.bridge.close)(self.handle.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn adjacent(name: &str) -> std::path::PathBuf {
        std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join(name)
    }

    fn rejected_no_update(variant: u32) {
        let name = format!("frame_capture_poison_{variant}.dll");
        let bridge = Bridge::load_exact(&adjacent(&name)).expect("poison fixture loads");
        let mut capture = bridge
            .open(Region {
                left: 0,
                top: 0,
                width: 5,
                height: 3,
            })
            .unwrap();
        let outcome = capture.poll(0);
        assert!(
            outcome.is_err(),
            "non-image metadata must be checked: {outcome:?}"
        );
    }

    #[test]
    fn protected_pointer_update_cannot_extend_cached_stability() {
        rejected_no_update(1);
    }

    #[test]
    fn inconsistent_pointer_count_cannot_extend_cached_stability() {
        rejected_no_update(2);
    }

    #[test]
    fn incompatible_pointer_layout_cannot_extend_cached_stability() {
        rejected_no_update(3);
    }

    #[test]
    fn protected_timeout_cannot_extend_cached_stability() {
        rejected_no_update(4);
    }

    #[test]
    fn real_adjacent_library_loads_and_rejects_invalid_regions() {
        let bridge = Bridge::load_adjacent().expect("verified adjacent capture DLL must load");
        for (width, height) in [(0, 3), (3, 0), (-1, 3), (4097, 3)] {
            assert!(
                bridge
                    .open(Region {
                        left: 400,
                        top: 329,
                        width,
                        height
                    })
                    .is_err()
            );
        }
    }

    #[test]
    fn a_missing_library_and_an_incompatible_abi_are_distinct_failures() {
        let missing = Bridge::load_exact(&adjacent("__altrun_missing.dll"))
            .err()
            .expect("missing library rejected");
        assert!(missing.contains("load"), "{missing}");
        let bad = Bridge::load_exact(&adjacent("frame_capture_badabi.dll"))
            .err()
            .expect("unsupported ABI rejected");
        assert!(bad.contains("ABI"), "{bad}");
    }

    #[test]
    fn pixels_and_timestamps_survive_an_owned_ffi_boundary() {
        let counters = Bridge::load_exact(&adjacent("frame_capture_fixture.dll"))
            .expect("fixture counters load");
        let raw = unsafe {
            windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                counters.module,
                c"altrun_fixture_closed".as_ptr().cast(),
            )
        }
        .unwrap();
        let closed: unsafe extern "C" fn() -> u32 = unsafe { std::mem::transmute_copy(&raw) };
        let before = unsafe { closed() };
        let bridge =
            Bridge::load_exact(&adjacent("frame_capture_fixture.dll")).expect("fixture loads");
        let weak = Rc::downgrade(&bridge);
        let mut capture = bridge
            .open(Region {
                left: 0,
                top: 0,
                width: 5,
                height: 3,
            })
            .unwrap();
        drop(bridge);
        assert!(weak.upgrade().is_some(), "capture keeps the DLL loaded");
        assert!(matches!(capture.poll(0).unwrap(), Poll::Timeout));
        assert!(matches!(capture.poll(0).unwrap(), Poll::PointerOnly));
        let Poll::Frame(frame) = capture.poll(0).unwrap() else {
            panic!("expected pixels")
        };
        assert_eq!(
            frame.stamp_after(10_000, 10_000_000).unwrap().elapsed_ns,
            1_000_000
        );
        let expected: Vec<u8> = (0..3)
            .flat_map(|y| (0..5).flat_map(move |x| [x, y, 127, 255]))
            .collect();
        assert_eq!(capture.pixels(), expected);
        assert!(capture.poll(51).is_err(), "poll timeout is bounded");
        drop(capture);
        assert_eq!(
            unsafe { closed() },
            before + 1,
            "native owner is closed exactly once"
        );
        assert!(
            weak.upgrade().is_none(),
            "DLL ownership is released with its last capture"
        );
    }
}
