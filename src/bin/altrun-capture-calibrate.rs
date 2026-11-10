//! Isolated observer calibration. No visible window, input or launcher access.
//! Pixel buffers and desktop hashes never leave memory. Timing only is saved.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod calibration {
    use std::{
        ffi::c_void,
        fs,
        hint::black_box,
        mem::{size_of, zeroed},
        path::{Path, PathBuf},
        ptr::null_mut,
        time::Instant,
    };
    use windows_sys::Win32::{
        Graphics::Gdi::*,
        UI::{HiDpi::*, WindowsAndMessaging::*},
    };

    struct Screen(HDC);
    impl Screen {
        fn new() -> Result<Self, String> {
            let dc = unsafe { GetDC(null_mut()) };
            if dc.is_null() {
                Err("Cannot acquire desktop DC".into())
            } else {
                Ok(Self(dc))
            }
        }
    }
    impl Drop for Screen {
        fn drop(&mut self) {
            unsafe {
                ReleaseDC(null_mut(), self.0);
            }
        }
    }

    #[derive(Clone, Copy)]
    enum Mode {
        Ddb,
        Dib,
    }
    impl Mode {
        fn name(self) -> &'static str {
            match self {
                Self::Ddb => "ddb_getdibits",
                Self::Dib => "dib_flush",
            }
        }
    }
    struct Buffer {
        dc: HDC,
        bitmap: HBITMAP,
        old: HGDIOBJ,
        bits: *mut c_void,
        info: BITMAPINFO,
        pixels: Vec<u8>,
        mode: Mode,
        width: i32,
        height: i32,
    }
    impl Buffer {
        fn new(compatible: HDC, width: i32, height: i32, mode: Mode) -> Result<Self, String> {
            if width <= 0 || height <= 0 || width > 4096 || height > 4096 {
                return Err("Invalid buffer dimensions".into());
            }
            unsafe {
                let mut info: BITMAPINFO = zeroed();
                info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = width;
                info.bmiHeader.biHeight = -height;
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB;
                let dc = CreateCompatibleDC(compatible);
                if dc.is_null() {
                    return Err("Cannot allocate memory DC".into());
                }
                let mut bits = null_mut();
                let bitmap = match mode {
                    Mode::Ddb => CreateCompatibleBitmap(compatible, width, height),
                    Mode::Dib => CreateDIBSection(
                        compatible,
                        &info,
                        DIB_RGB_COLORS,
                        &mut bits,
                        null_mut(),
                        0,
                    ),
                };
                if bitmap.is_null() || (matches!(mode, Mode::Dib) && bits.is_null()) {
                    if !bitmap.is_null() {
                        DeleteObject(bitmap);
                    }
                    DeleteDC(dc);
                    return Err("Cannot allocate bitmap".into());
                }
                let old = SelectObject(dc, bitmap);
                if old.is_null() || old as isize == -1 {
                    DeleteObject(bitmap);
                    DeleteDC(dc);
                    return Err("Cannot select bitmap".into());
                }
                Ok(Self {
                    dc,
                    bitmap,
                    old,
                    bits,
                    info,
                    pixels: vec![0; width as usize * height as usize * 4],
                    mode,
                    width,
                    height,
                })
            }
        }
        fn capture(
            &mut self,
            source: HDC,
            x: i32,
            y: i32,
            desktop: bool,
        ) -> Result<Timing, String> {
            let start = Instant::now();
            unsafe {
                let rop = SRCCOPY | if desktop { CAPTUREBLT } else { 0 };
                if BitBlt(self.dc, 0, 0, self.width, self.height, source, x, y, rop) == 0 {
                    return Err("Pixel copy failed".into());
                }
            }
            let copy_ns = start.elapsed().as_nanos();
            unsafe {
                match self.mode {
                    Mode::Ddb => {
                        SelectObject(self.dc, self.old);
                        let rows = GetDIBits(
                            source,
                            self.bitmap,
                            0,
                            self.height as u32,
                            self.pixels.as_mut_ptr() as *mut c_void,
                            &mut self.info,
                            DIB_RGB_COLORS,
                        );
                        SelectObject(self.dc, self.bitmap);
                        if rows != self.height {
                            return Err("Incomplete DDB read".into());
                        }
                    }
                    Mode::Dib => {
                        // Required before directly accessing DIB bits; not a presentation barrier.
                        if GdiFlush() == 0 {
                            return Err("DIB synchronization failed".into());
                        }
                        let bytes =
                            std::slice::from_raw_parts(self.bits as *const u8, self.pixels.len());
                        self.pixels.copy_from_slice(bytes);
                    }
                }
            }
            let read_end_ns = start.elapsed().as_nanos();
            let hash = self.pixels.iter().fold(0xcbf29ce484222325_u64, |h, &b| {
                (h ^ b as u64).wrapping_mul(0x100000001b3)
            });
            black_box(hash);
            let total_ns = start.elapsed().as_nanos();
            Ok(Timing {
                copy_ns,
                read_ns: read_end_ns - copy_ns,
                hash_ns: total_ns - read_end_ns,
                total_ns,
            })
        }
    }
    impl Drop for Buffer {
        fn drop(&mut self) {
            unsafe {
                GdiFlush();
                SelectObject(self.dc, self.old);
                DeleteObject(self.bitmap);
                DeleteDC(self.dc);
            }
        }
    }
    struct Timing {
        copy_ns: u128,
        read_ns: u128,
        hash_ns: u128,
        total_ns: u128,
    }
    impl Timing {
        fn json(&self, mode: Mode, pair: usize, order: usize) -> String {
            format!(
                "{{\"mode\":\"{}\",\"pair\":{pair},\"order\":{order},\"copy_ns\":{},\"read_ns\":{},\"hash_ns\":{},\"total_ns\":{}}}",
                mode.name(),
                self.copy_ns,
                self.read_ns,
                self.hash_ns,
                self.total_ns
            )
        }
    }
    pub fn self_check() -> Result<(), String> {
        let screen = Screen::new()?;
        let source = Buffer::new(screen.0, 37, 19, Mode::Dib)?;
        let mut ddb = Buffer::new(screen.0, 37, 19, Mode::Ddb)?;
        let mut dib = Buffer::new(screen.0, 37, 19, Mode::Dib)?;
        for iteration in 0..8 {
            let white = iteration % 2 != 0;
            unsafe {
                if PatBlt(
                    source.dc,
                    0,
                    0,
                    37,
                    19,
                    if white { WHITENESS } else { BLACKNESS },
                ) == 0
                    || SetPixelV(source.dc, 0, 0, 0x000000ff) == 0
                    || SetPixelV(source.dc, 36, 18, 0x0000ff00) == 0
                    || SetPixelV(source.dc, 5, 7, 0x00ff0000) == 0
                    || GdiFlush() == 0
                {
                    return Err("Cannot draw controlled pattern".into());
                }
            }
            for target in [&mut ddb, &mut dib] {
                target.capture(source.dc, 0, 0, false)?;
                for y in 0..19 {
                    for x in 0..37 {
                        let expected = match (x, y) {
                            (0, 0) => [0, 0, 255],
                            (36, 18) => [0, 255, 0],
                            (5, 7) => [255, 0, 0],
                            _ => [if white { 255 } else { 0 }; 3],
                        };
                        let offset = (y * 37 + x) * 4;
                        if target.pixels[offset..offset + 3] != expected {
                            return Err("Controlled RGB pattern mismatch".into());
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn validate_output(output: &Path) -> Result<(), String> {
        if !output.is_absolute()
            || !output
                .to_string_lossy()
                .to_ascii_lowercase()
                .starts_with("d:\\")
        {
            return Err("Output must stay on D:".into());
        }
        Ok(())
    }
    pub fn run(output: &Path, synthetic_only: bool) -> Result<(), String> {
        validate_output(output)?;
        self_check()?;
        let mut rows = Vec::new();
        if !synthetic_only {
            unsafe {
                SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            }
            let (x, y, width, height) = (400, 329, 840, 459);
            let (vx, vy, vw, vh) = unsafe {
                (
                    GetSystemMetrics(SM_XVIRTUALSCREEN),
                    GetSystemMetrics(SM_YVIRTUALSCREEN),
                    GetSystemMetrics(SM_CXVIRTUALSCREEN),
                    GetSystemMetrics(SM_CYVIRTUALSCREEN),
                )
            };
            if x < vx || y < vy || x + width > vx + vw || y + height > vy + vh {
                return Err("Calibration region outside virtual desktop".into());
            }
            let screen = Screen::new()?;
            let mut buffers = [
                Buffer::new(screen.0, width, height, Mode::Ddb)?,
                Buffer::new(screen.0, width, height, Mode::Dib)?,
            ];
            for pair in 0..210 {
                for order in 0..2 {
                    let which = (pair + order) % 2;
                    let buffer = &mut buffers[which];
                    let timing = buffer.capture(screen.0, x, y, true)?;
                    if pair >= 10 {
                        rows.push(timing.json(buffer.mode, pair - 10, order));
                    }
                }
            }
        }
        let text = format!(
            "{{\"measurement_kind\":\"capture-calibration\",\"decision\":\"pending\",\"content_verified\":false,\"synthetic_rgb_verified\":true,\"desktop_sampled\":{},\"samples\":[],\"timings\":[{}],\"limitations\":[\"Observer component overhead only\",\"No window validation overhead included\",\"No launcher or hardware presentation evidence\",\"Desktop pixels and hashes are not saved\"]}}",
            !synthetic_only,
            rows.join(",")
        );
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(output, text).map_err(|e| e.to_string())
    }
    pub fn entry() -> Result<(), String> {
        let args: Vec<String> = std::env::args().collect();
        if args.len() > 1 {
            // The only CLI mode is controlled pixels, never an implicit desktop run.
            if args.len() != 4 || args[1] != "--synthetic-only" || args[2] != "--output" {
                return Err("Expected --synthetic-only --output D:\\path".into());
            }
            return run(Path::new(&args[3]), true);
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let folder = exe.parent().ok_or("Missing executable directory")?;
        let output: PathBuf = folder.join("calibration-result.json");
        validate_output(&output)?;
        if fs::read(folder.join("capture-calibration.profile"))
            .map_err(|_| "Missing explicit calibration profile")?
            != b"ALTRUN-CAPTURE-CALIBRATION\n"
        {
            return Err("Invalid calibration profile".into());
        }
        let result = run(&output, false);
        fs::write(
            folder.join("calibration-status.txt"),
            match &result {
                Ok(()) => "Calibration complete; performance gate pending.".to_string(),
                Err(error) => format!("Calibration failed: {error}"),
            },
        )
        .map_err(|e| e.to_string())?;
        result
    }
}

fn main() {
    #[cfg(windows)]
    if let Err(error) = calibration::entry() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn both_capture_paths_read_changed_rgb_pixels_in_top_down_order() {
        assert!(
            super::calibration::self_check().is_ok(),
            "both paths must read the controlled GDI pattern"
        );
    }
}
