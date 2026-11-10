//! An owned, versioned boundary to the independent desktop-frame observer.
use crate::desktop_frame::DesktopFrame;
use std::{path::Path, rc::Rc};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Region { pub left: i32, pub top: i32, pub width: i32, pub height: i32 }
pub struct Bridge { module: windows_sys::Win32::Foundation::HMODULE }
pub struct Capture;
#[derive(Debug)]
pub enum Poll { Timeout, PointerOnly, Frame(DesktopFrame) }

impl Bridge {
    pub fn load_adjacent() -> Result<Rc<Self>, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        Self::load_exact(&exe.parent().ok_or("Missing executable parent")?.join("altrun-frame-capture.dll"))
    }
    fn load_exact(_path: &Path) -> Result<Rc<Self>, String> { Err("Capture bridge not implemented".into()) }
    pub fn open(self: &Rc<Self>, _region: Region) -> Result<Capture, String> { Err("Capture owner not implemented".into()) }
}
impl Capture {
    pub fn poll(&mut self, _timeout_ms: u32) -> Result<Poll, String> { Err("Capture poll not implemented".into()) }
    pub fn pixels(&self) -> &[u8] { &[] }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn adjacent(name: &str) -> std::path::PathBuf { std::env::current_exe().unwrap().parent().unwrap().join(name) }

    #[test]
    fn real_adjacent_library_loads_and_rejects_invalid_regions() {
        let bridge = Bridge::load_adjacent().expect("verified adjacent capture DLL must load");
        for (width, height) in [(0, 3), (3, 0), (-1, 3), (4097, 3)] {
            assert!(bridge.open(Region { left: 400, top: 329, width, height }).is_err());
        }
    }

    #[test]
    fn a_missing_library_and_an_incompatible_abi_are_distinct_failures() {
        let missing = Bridge::load_exact(&adjacent("__altrun_missing.dll")).err().expect("missing library rejected");
        assert!(missing.contains("load"), "{missing}");
        let bad = Bridge::load_exact(&adjacent("frame_capture_badabi.dll")).err().expect("unsupported ABI rejected");
        assert!(bad.contains("ABI"), "{bad}");
    }

    #[test]
    fn pixels_and_timestamps_survive_an_owned_ffi_boundary() {
        let counters = Bridge::load_exact(&adjacent("frame_capture_fixture.dll")).expect("fixture counters load");
        let raw = unsafe { windows_sys::Win32::System::LibraryLoader::GetProcAddress(counters.module, c"altrun_fixture_closed".as_ptr().cast()) }.unwrap();
        let closed: unsafe extern "C" fn() -> u32 = unsafe { std::mem::transmute_copy(&raw) };
        let before = unsafe { closed() };
        let bridge = Bridge::load_exact(&adjacent("frame_capture_fixture.dll")).expect("fixture loads");
        let weak = Rc::downgrade(&bridge);
        let mut capture = bridge.open(Region { left: 0, top: 0, width: 5, height: 3 }).unwrap();
        drop(bridge);
        assert!(weak.upgrade().is_some(), "capture keeps the DLL loaded");
        assert!(matches!(capture.poll(0).unwrap(), Poll::Timeout));
        assert!(matches!(capture.poll(0).unwrap(), Poll::PointerOnly));
        let Poll::Frame(frame) = capture.poll(0).unwrap() else { panic!("expected pixels") };
        assert_eq!(frame.stamp_after(10_000, 10_000_000).unwrap().elapsed_ns, 1_000_000);
        let expected: Vec<u8> = (0..3).flat_map(|y| (0..5).flat_map(move |x| [x, y, 127, 255])).collect();
        assert_eq!(capture.pixels(), expected);
        assert!(capture.poll(51).is_err(), "poll timeout is bounded");
        drop(capture);
        assert_eq!(unsafe { closed() }, before + 1, "native owner is closed exactly once");
        assert!(weak.upgrade().is_none(), "DLL ownership is released with its last capture");
    }
}
