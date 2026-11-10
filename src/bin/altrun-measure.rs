//! A narrowly scoped local test instrument. No Enter key or command execution.
//! Each trial is compared with its own final sampled frame, since legacy
//! equal-rank results can change order. Report sampling bounds, never hardware
//! timestamps, physical keyboard latency, IME coverage, or a complete gate pass.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#[cfg(windows)]
mod windows {
    use altrun_probe::observation::{Bounds, Frame, Rejected, settled_bounds};
    use altrun_probe::capture_bridge::{Bridge, Capture as NativeCapture, Poll as NativePoll, Region};
    use altrun_probe::desktop_observation::{self, DesktopBounds, DesktopObservation, rgb_hash};
    use std::{
        cell::RefCell,
        ffi::c_void,
        fs,
        mem::{size_of, zeroed},
        ptr::{null, null_mut},
        rc::Rc,
        thread,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::*,
        Graphics::{Dwm::*, Gdi::*},
        System::{LibraryLoader::GetModuleHandleW, Performance::*},
        UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    };

    thread_local! {
        static CAPTURE_BRIDGE: RefCell<Option<Rc<Bridge>>> = const { RefCell::new(None) };
    }
    fn qpc() -> Result<i64, String> {
        let mut value = 0;
        if unsafe { QueryPerformanceCounter(&mut value) } == 0 { return Err("Cannot read QPC".into()); }
        Ok(value)
    }
    fn qpc_frequency() -> Result<i64, String> {
        let mut value = 0;
        if unsafe { QueryPerformanceFrequency(&mut value) } == 0 || value <= 0 { return Err("Cannot read QPC frequency".into()); }
        Ok(value)
    }
    fn ns_since(start: i64, end: i64, frequency: i64) -> Result<u128, String> {
        if start < 0 || end < start || frequency <= 0 { return Err("Invalid QPC observation interval".into()); }
        Ok(((i128::from(end) - i128::from(start)) * 1_000_000_000 / i128::from(frequency)) as u128)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn class(hwnd: HWND) -> String {
        unsafe {
            let mut b = [0_u16; 128];
            let n = GetClassNameW(hwnd, b.as_mut_ptr(), 128);
            String::from_utf16_lossy(&b[..n as usize])
        }
    }
    unsafe extern "system" fn enumerate(hwnd: HWND, param: LPARAM) -> i32 {
        unsafe {
            (&mut *(param as *mut Vec<HWND>)).push(hwnd);
        }
        1
    }
    fn find_target(pid: u32, legacy: bool) -> Result<HWND, String> {
        let mut all = Vec::<HWND>::new();
        unsafe {
            EnumWindows(Some(enumerate), &mut all as *mut _ as isize);
        }
        let matches: Vec<_> = all
            .into_iter()
            .filter(|&h| {
                let mut p = 0;
                unsafe {
                    GetWindowThreadProcessId(h, &mut p);
                }
                p == pid
                    && class(h)
                        == if legacy {
                            "TALTRunForm"
                        } else {
                            "ALTRun.Native.PerformanceProbe"
                        }
            })
            .collect();
        if matches.len() != 1 {
            return Err("Expected one verified ALTRun target window".into());
        }
        Ok(matches[0])
    }
    fn children(hwnd: HWND) -> Result<(HWND, HWND), String> {
        let mut all = Vec::<HWND>::new();
        unsafe {
            EnumChildWindows(hwnd, Some(enumerate), &mut all as *mut _ as isize);
        }
        let edit = all.iter().copied().find(|&h| {
            let s = unsafe { GetWindowLongPtrW(h, GWL_STYLE) } as u32;
            class(h).to_lowercase().ends_with("edit")
                && s & ES_READONLY as u32 == 0
                && s & WS_DISABLED == 0
                && s & WS_VISIBLE != 0
        });
        let list = all
            .iter()
            .copied()
            .find(|&h| class(h).to_lowercase().ends_with("listbox"));
        edit.zip(list)
            .ok_or("Cannot identify native editable query/list controls".into())
    }
    fn set_query(edit: HWND, q: &str) -> Result<(), String> {
        unsafe {
            let b = wide(q);
            let mut result = 0;
            SetLastError(0);
            if SendMessageTimeoutW(
                edit,
                WM_SETTEXT,
                0,
                b.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                2000,
                &mut result,
            ) == 0
            {
                return Err(format!(
                    "WM_SETTEXT failed (Win32 error {})",
                    GetLastError()
                ));
            }
        }
        Ok(())
    }
    fn message(hwnd: HWND, msg: u32, w: usize, l: isize) -> Result<usize, String> {
        let mut result = 0;
        if unsafe {
            SetLastError(0);
            SendMessageTimeoutW(
                hwnd,
                msg,
                w,
                l,
                SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT,
                2000,
                &mut result,
            )
        } == 0
        {
            return Err(format!(
                "Message {msg:#x} failed (Win32 error {})",
                unsafe { GetLastError() }
            ));
        }
        Ok(result)
    }
    fn get_query(edit: HWND) -> Result<String, String> {
        let length = message(edit, WM_GETTEXTLENGTH, 0, 0)?;
        if length > 32768 {
            return Err("Original query too long to safely preserve".into());
        }
        let mut b = vec![0_u16; length + 1];
        let n = message(edit, WM_GETTEXT, b.len(), b.as_mut_ptr() as isize)?;
        if n > length {
            return Err("Query changed while preserving it".into());
        }
        Ok(String::from_utf16_lossy(&b[..n]))
    }
    fn foreground_pid() -> u32 {
        unsafe {
            let mut p = 0;
            GetWindowThreadProcessId(GetForegroundWindow(), &mut p);
            p
        }
    }
    fn input(keys: &[(u16, bool)]) -> Result<(), String> {
        let values: Vec<INPUT> = keys
            .iter()
            .map(|&(key, up)| {
                let mut i: INPUT = unsafe { zeroed() };
                i.r#type = INPUT_KEYBOARD;
                i.Anonymous.ki = KEYBDINPUT {
                    wVk: key,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                };
                i
            })
            .collect();
        if unsafe {
            SendInput(
                values.len() as u32,
                values.as_ptr(),
                size_of::<INPUT>() as i32,
            )
        } != values.len() as u32
        {
            return Err("SendInput failed; no fallback activation was used".into());
        }
        Ok(())
    }
    fn wake(legacy: bool) -> Result<(), String> {
        if [VK_CONTROL, VK_SHIFT, VK_MENU]
            .iter()
            .any(|&k| unsafe { GetAsyncKeyState(k as i32) } < 0)
        {
            return Err("Release modifier keys before measurement".into());
        }
        if legacy {
            input(&[
                (VK_MENU, false),
                (0x52, false),
                (0x52, true),
                (VK_MENU, true),
            ])
        } else {
            input(&[
                (VK_CONTROL, false),
                (VK_MENU, false),
                (0x52, false),
                (0x52, true),
                (VK_MENU, true),
                (VK_CONTROL, true),
            ])
        }
    }
    fn wait_visible(hwnd: HWND, visible: bool) -> Result<(), String> {
        let start = Instant::now();
        while (unsafe { IsWindowVisible(hwnd) } != 0) != visible {
            if start.elapsed() > Duration::from_secs(2) {
                return Err("Target did not reach requested visibility".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    struct Capture {
        screen: HDC,
        memory: HDC,
        bitmap: HBITMAP,
        old: HGDIOBJ,
        rect: RECT,
        pixels: Vec<u8>,
        info: BITMAPINFO,
        native: Option<NativeCapture>,
    }
    impl Capture {
        fn new(rect: RECT) -> Result<Self, String> {
            unsafe {
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;
                if width <= 0 || height <= 0 || width > 4096 || height > 4096 {
                    return Err("Invalid results capture region".into());
                }
                let screen = GetDC(null_mut());
                let memory = CreateCompatibleDC(screen);
                let bitmap = CreateCompatibleBitmap(screen, width, height);
                if screen.is_null() || memory.is_null() || bitmap.is_null() {
                    if !bitmap.is_null() {
                        DeleteObject(bitmap);
                    }
                    if !memory.is_null() {
                        DeleteDC(memory);
                    }
                    if !screen.is_null() {
                        ReleaseDC(null_mut(), screen);
                    }
                    return Err("Cannot allocate capture buffers".into());
                }
                let old = SelectObject(memory, bitmap);
                let mut info: BITMAPINFO = zeroed();
                info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = width;
                info.bmiHeader.biHeight = -height;
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB;
                let mut capture = Self {
                    screen,
                    memory,
                    bitmap,
                    old,
                    rect,
                    pixels: vec![0; width as usize * height as usize * 4],
                    info,
                    native: None,
                };
                capture.native = CAPTURE_BRIDGE.with(|cell| {
                    cell.borrow().as_ref().map(|bridge| bridge.open(Region {
                        left: rect.left, top: rect.top, width, height,
                    })).transpose()
                })?;
                Ok(capture)
            }
        }
        fn hash(&mut self) -> Result<u64, String> {
            unsafe {
                let width = self.rect.right - self.rect.left;
                let height = self.rect.bottom - self.rect.top;
                if BitBlt(
                    self.memory,
                    0,
                    0,
                    width,
                    height,
                    self.screen,
                    self.rect.left,
                    self.rect.top,
                    SRCCOPY | CAPTUREBLT,
                ) == 0
                {
                    return Err("Desktop pixel copy failed".into());
                }
                // GetDIBits requires the bitmap to be deselected from its DC.
                SelectObject(self.memory, self.old);
                let rows = GetDIBits(
                    self.screen,
                    self.bitmap,
                    0,
                    height as u32,
                    self.pixels.as_mut_ptr() as *mut c_void,
                    &mut self.info,
                    DIB_RGB_COLORS,
                );
                SelectObject(self.memory, self.bitmap);
                if rows != height {
                    return Err("Incomplete desktop pixel read".into());
                }
                self.hash_pixels()
            }
        }
        fn hash_pixels(&self) -> Result<u64, String> {
            if self.native.is_some() { return rgb_hash(&self.pixels).map_err(str::to_string); }
            Ok(self.pixels.iter().fold(0xcbf29ce484222325_u64, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3)))
        }
        fn poll_native(&mut self, timeout: u32) -> Result<NativePoll, String> {
            let native = self.native.as_mut().ok_or("Desktop capture is unavailable")?;
            let result = native.poll(timeout)?;
            if matches!(result, NativePoll::Frame(_)) { self.pixels.copy_from_slice(native.pixels()); }
            Ok(result)
        }
        fn calibrate_backend(&mut self) -> Result<(), String> {
            let Some(native) = self.native.as_mut() else { return Ok(()); };
            let start = Instant::now();
            loop {
                if matches!(native.poll(50)?, NativePoll::Frame(_)) {
                    if self.pixels.len() != native.pixels().len() { return Err("Capture calibration size mismatch".into()); }
                    let differences = self.pixels.chunks_exact(4).zip(native.pixels().chunks_exact(4)).filter(|(a, b)| a[..3] != b[..3]).count();
                    if differences != 0 { return Err(format!("GDI/DXGI RGB calibration mismatch: {differences} pixels")); }
                    return Ok(());
                }
                if start.elapsed() >= Duration::from_secs(2) { return Err("No desktop image for backend calibration".into()); }
            }
        }
        fn prime_native(&mut self) -> Result<(), String> {
            let Some(native) = self.native.as_mut() else { return Ok(()); };
            for _ in 0..16 {
                if matches!(native.poll(0)?, NativePoll::Timeout) { return Ok(()); }
            }
            Err("Cannot drain desktop frames before the action".into())
        }
        fn part_hash(&self, region: RECT) -> Result<u64, String> {
            if region.left < self.rect.left
                || region.top < self.rect.top
                || region.right > self.rect.right
                || region.bottom > self.rect.bottom
                || region.right <= region.left
                || region.bottom <= region.top
            {
                return Err("List pixels are outside the observed canvas".into());
            }
            let stride = (self.rect.right - self.rect.left) as usize * 4;
            let left = (region.left - self.rect.left) as usize * 4;
            let right = (region.right - self.rect.left) as usize * 4;
            let top = (region.top - self.rect.top) as usize;
            let bottom = (region.bottom - self.rect.top) as usize;
            let mut hash = 0xcbf29ce484222325_u64;
            for row in top..bottom {
                for pixel in self.pixels[row * stride + left..row * stride + right].chunks_exact(4) {
                    let channels = if self.native.is_some() { &pixel[..3] } else { pixel };
                    for &value in channels { hash = (hash ^ u64::from(value)).wrapping_mul(0x100000001b3); }
                }
            }
            Ok(hash)
        }
    }
    impl Drop for Capture {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.memory, self.old);
                DeleteObject(self.bitmap);
                DeleteDC(self.memory);
                ReleaseDC(null_mut(), self.screen);
            }
        }
    }
    const QUERIES: [&str; 5] = ["", "a", "e", "o", "r"];
    fn rectangle(hwnd: HWND) -> Result<RECT, String> {
        let mut r: RECT = unsafe { zeroed() };
        if unsafe { GetWindowRect(hwnd, &mut r) } == 0 {
            return Err("Cannot read results geometry".into());
        }
        Ok(r)
    }
    fn same_rect(a: RECT, b: RECT) -> bool {
        a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom
    }
    #[derive(Debug, Default, PartialEq, Eq)]
    struct PixelDiff {
        compared: bool,
        list: usize,
        parent: usize,
        outside: usize,
        alpha_only: usize,
        bounds: Option<[i32; 4]>,
    }
    fn pixel_diff(
        expected: &[u8],
        actual: &[u8],
        capture: RECT,
        list: RECT,
        parent: RECT,
    ) -> PixelDiff {
        if capture.right <= capture.left || capture.bottom <= capture.top {
            return PixelDiff::default();
        }
        let width = (capture.right as i64 - capture.left as i64) as usize;
        let height = (capture.bottom as i64 - capture.top as i64) as usize;
        let size = width.checked_mul(height).and_then(|n| n.checked_mul(4));
        if size != Some(expected.len()) || expected.len() != actual.len() {
            return PixelDiff::default();
        }
        let mut diff = PixelDiff {
            compared: true,
            ..PixelDiff::default()
        };
        let (expected_pixels, _) = expected.as_chunks::<4>();
        let (actual_pixels, _) = actual.as_chunks::<4>();
        let pairs = expected_pixels.iter().zip(actual_pixels);
        for (i, (a, b)) in pairs.enumerate() {
            if a == b {
                continue;
            }
            let x = capture.left + (i % width) as i32;
            let y = capture.top + (i / width) as i32;
            let inside = |r: RECT| x >= r.left && x < r.right && y >= r.top && y < r.bottom;
            if inside(list) {
                diff.list += 1;
            } else if inside(parent) {
                diff.parent += 1;
            } else {
                diff.outside += 1;
            }
            if a[..3] == b[..3] {
                diff.alpha_only += 1;
            }
            if let Some(bounds) = &mut diff.bounds {
                bounds[0] = bounds[0].min(x);
                bounds[1] = bounds[1].min(y);
                bounds[2] = bounds[2].max(x + 1);
                bounds[3] = bounds[3].max(y + 1);
            } else {
                diff.bounds = Some([x, y, x + 1, y + 1]);
            }
        }
        diff
    }
    struct Report {
        output: std::path::PathBuf,
        kind: &'static str,
        stage: &'static str,
        target: &'static str,
        iteration: usize,
        query_id: usize,
        samples: Vec<String>,
        current_trace: Vec<Frame>,
        current_desktop_trace: Vec<DesktopObservation>,
        clock_hz: i64,
        action_qpc: Option<i64>,
        dispatch_qpc: Option<i64>,
        complete: bool,
        diagnostic_windows: String,
    }
    impl Report {
        fn new(output: std::path::PathBuf) -> Self {
            Self {
                output,
                kind: "pixel-settling-observations",
                stage: "original_identity",
                target: "original",
                iteration: 0,
                query_id: 0,
                samples: Vec::new(),
                current_trace: Vec::new(),
                current_desktop_trace: Vec::new(),
                clock_hz: 0,
                action_qpc: None,
                dispatch_qpc: None,
                complete: false,
                diagnostic_windows: "{}".into(),
            }
        }
        fn save(&self) -> Result<(), String> {
            let status = if self.complete {
                "observed"
            } else {
                "rejected"
            };
            let rows = self.samples.join(",");
            let trace = trace_json(&self.current_trace);
            let desktop_trace = desktop_trace_json(&self.current_desktop_trace);
            let clock_hz = self.clock_hz;
            let action_qpc = self.action_qpc.map_or_else(|| "null".to_string(), |v| v.to_string());
            let dispatch_qpc = self.dispatch_qpc.map_or_else(|| "null".to_string(), |v| v.to_string());
            let text = format!(
                "{{\"measurement_kind\":\"{}\",\"decision\":\"pending\",\"content_verified\":false,\"session_status\":\"{}\",\"stage\":\"{}\",\"target\":\"{}\",\"iteration\":{},\"query_id\":{},\"samples\":[{}],\"current_trace\":[{}],\"current_desktop_trace\":[{desktop_trace}],\"action\":{{\"frequency\":{clock_hz},\"request_qpc\":{action_qpc},\"dispatch_complete_qpc\":{dispatch_qpc}}},\"diagnostic_windows\":{},\"limitations\":[\"Final sampled pixels are not an independent content oracle\",\"First matching observation and final stabilization are separate\",\"Sampling is not hardware presentation or physical input\",\"Rejected, synthetic or content-unverified sessions cannot establish parity\"]}}",
                self.kind,
                status,
                self.stage,
                self.target,
                self.iteration,
                self.query_id,
                rows,
                trace,
                self.diagnostic_windows
            );
            if let Some(parent) = self.output.parent() {
                fs::create_dir_all(parent).map_err(|_| "Cannot create evidence folder")?;
            }
            fs::write(&self.output, text).map_err(|_| "Cannot save pending evidence".into())
        }
    }
    impl Drop for Report {
        fn drop(&mut self) {
            if let Err(error) = self.save() {
                eprintln!("{error}");
            }
        }
    }
    fn trace_json(frames: &[Frame]) -> String {
        frames.iter().map(|f| format!(
            "{{\"began_ns\":{},\"ended_ns\":{},\"hash\":{},\"list_hash\":{},\"ready\":{},\"unoccluded\":{}}}",
            f.began_ns, f.ended_ns, f.hash, f.list_hash, f.ready, f.unoccluded
        )).collect::<Vec<_>>().join(",")
    }
    fn desktop_trace_json(frames: &[DesktopObservation]) -> String {
        frames.iter().map(|f| {
            let image = f.image_ns.map_or_else(|| "null".to_string(), |n| n.to_string());
            let o = f.observed;
            format!("{{\"began_ns\":{},\"ended_ns\":{},\"image_ns\":{image},\"fresh\":{},\"accumulated_frames\":{},\"protected_content\":{},\"hash\":{},\"list_hash\":{},\"ready\":{},\"unoccluded\":{}}}", o.began_ns, o.ended_ns, f.fresh, f.accumulated_frames, f.protected_content, o.hash, o.list_hash, o.ready, o.unoccluded)
        }).collect::<Vec<_>>().join(",")
    }
    fn desktop_sample_json(report: &Report, metric: &str, bounds: &DesktopBounds) -> Result<String, String> {
        let request = report.action_qpc.ok_or("Missing action QPC")?;
        let dispatch = report.dispatch_qpc.ok_or("Missing dispatch QPC")?;
        let dispatch_ns = ns_since(request, dispatch, report.clock_hz)?;
        let trace = desktop_trace_json(&report.current_desktop_trace);
        let target = report.target;
        let iteration = report.iteration;
        let query_id = report.query_id;
        let image = bounds.image_update_ns;
        let ready_lower = bounds.first_ready_lower_ns;
        let ready_upper = bounds.first_ready_upper_ns;
        let observed_upper = bounds.pixels.upper_ns;
        let span = bounds.pixels.stable_span_ns;
        let gap = bounds.pixels.maximum_gap_ns;
        let capture = bounds.pixels.maximum_capture_ns;
        let sampling = bounds.pixels.maximum_sampling_span_ns;
        Ok(format!("{{\"target\":\"{target}\",\"iteration\":{iteration},\"query_id\":{query_id},\"metric\":\"{metric}\",\"content_verified\":false,\"request_to_dispatch_ns\":{dispatch_ns},\"desktop_image_update_ns\":{image},\"ready_observed_lower_ns\":{ready_lower},\"ready_observed_upper_ns\":{ready_upper},\"first_match_observed_upper_ns\":{observed_upper},\"stable_observed_span_ns\":{span},\"maximum_gap_ns\":{gap},\"maximum_capture_ns\":{capture},\"maximum_sampling_span_ns\":{sampling},\"trace\":[{trace}]}}"))
    }
    fn sample_json(
        target: &str,
        iteration: usize,
        query_id: usize,
        metric: &str,
        bounds: &Bounds,
        frames: &[Frame],
    ) -> String {
        let trace = trace_json(frames);
        format!(
            "{{\"target\":\"{target}\",\"iteration\":{iteration},\"query_id\":{query_id},\"metric\":\"{metric}\",\"content_verified\":false,\"lower_ns\":{},\"upper_ns\":{},\"settled_upper_ns\":{},\"stable_span_ns\":{},\"maximum_gap_ns\":{},\"maximum_capture_ns\":{},\"maximum_sampling_span_ns\":{},\"sample_count\":{},\"trace\":[{trace}]}}",
            bounds.lower_ns,
            bounds.upper_ns,
            bounds.settled_upper_ns,
            bounds.stable_span_ns,
            bounds.maximum_gap_ns,
            bounds.maximum_capture_ns,
            bounds.maximum_sampling_span_ns,
            bounds.sample_count
        )
    }
    fn window_metadata(hwnd: HWND, canvas: RECT) -> Result<String, String> {
        let rect = rectangle(hwnd)?;
        let mut overlap: RECT = unsafe { zeroed() };
        let intersects = unsafe { IntersectRect(&mut overlap, &canvas, &rect) } != 0;
        let mut pid = 0;
        let mut cloaked: u32 = 0;
        let mut color = 0;
        let mut alpha = 0;
        let mut flags = 0;
        let mut region_bounds: RECT = unsafe { zeroed() };
        let (cloak_hr, layered_ok, region_kind) = unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            let hr = DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED as u32,
                &mut cloaked as *mut _ as *mut c_void,
                size_of::<u32>() as u32,
            );
            let layered = GetLayeredWindowAttributes(hwnd, &mut color, &mut alpha, &mut flags) != 0;
            let region = CreateRectRgn(0, 0, 0, 0);
            let kind = if region.is_null() {
                -1
            } else {
                let kind = GetWindowRgn(hwnd, region);
                GetRgnBox(region, &mut region_bounds);
                DeleteObject(region);
                kind
            };
            (hr, layered, kind)
        };
        let class_hex: String = class(hwnd)
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(format!(
            "{{\"pid\":{pid},\"class_utf8_hex\":\"{class_hex}\",\"visible\":{},\"ex_style\":{},\"rect\":[{},{},{},{}],\"intersects_canvas\":{intersects},\"cloak_hresult\":{cloak_hr},\"cloaked\":{cloaked},\"layered_query_ok\":{layered_ok},\"alpha\":{alpha},\"layered_flags\":{flags},\"region_kind\":{region_kind},\"region_bounds\":[{},{},{},{}]}}",
            unsafe { IsWindowVisible(hwnd) } != 0,
            unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            region_bounds.left,
            region_bounds.top,
            region_bounds.right,
            region_bounds.bottom
        ))
    }
    // Metadata only: no ShowWindow, input, foreground change, capture or launcher.
    fn inspect_window_stack(output: std::path::PathBuf) -> Result<(), String> {
        let mut report = Report::new(output);
        report.kind = "observer-window-stack";
        report.target = "synthetic-hidden-window";
        report.stage = "window_stack_setup";
        let dpi_context_ok =
            unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
                != 0;
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                wide("STATIC").as_ptr(),
                wide("").as_ptr(),
                WS_POPUP | WS_BORDER,
                400,
                220,
                420,
                284,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            )
        };
        if hwnd.is_null() {
            return Err("Cannot create hidden diagnostic window".into());
        }
        struct Hidden(HWND);
        impl Drop for Hidden {
            fn drop(&mut self) {
                unsafe {
                    DestroyWindow(self.0);
                }
            }
        }
        let _hidden = Hidden(hwnd);
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        unsafe {
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                (420 * dpi / 96) as i32,
                (284 * dpi / 96) as i32,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        let mut origin = POINT { x: 0, y: 0 };
        if unsafe { ClientToScreen(hwnd, &mut origin) } == 0 {
            return Err("Cannot locate diagnostic canvas".into());
        }
        let parent = rectangle(hwnd)?;
        let canvas = RECT {
            left: parent.left,
            top: origin.y + (54 * dpi / 96) as i32,
            right: parent.right,
            bottom: parent.bottom,
        };
        let root = window_metadata(hwnd, canvas)?;
        let mut above = unsafe { GetWindow(hwnd, GW_HWNDPREV) };
        let mut checked = 0;
        let mut rows = Vec::new();
        while !above.is_null() {
            checked += 1;
            if checked > 4096 {
                return Err("Unstable diagnostic window order".into());
            }
            if unsafe { IsWindowVisible(above) } != 0 && unsafe { IsIconic(above) } == 0 {
                rows.push(window_metadata(above, canvas)?);
            }
            above = unsafe { GetWindow(above, GW_HWNDPREV) };
        }
        report.diagnostic_windows = format!(
            "{{\"diagnostic_source\":\"equivalent-hidden-topmost-window\",\"dpi_context_ok\":{dpi_context_ok},\"dpi\":{dpi},\"root\":{root},\"canvas\":[{},{},{},{}],\"checked\":{checked},\"windows\":[{}]}}",
            canvas.left,
            canvas.top,
            canvas.right,
            canvas.bottom,
            rows.join(",")
        );
        report.stage = "window_stack_inspected";
        report.complete = true;
        report.save()
    }
    fn unoccluded(hwnd: HWND, canvas: RECT) -> Result<bool, String> {
        let mut above = unsafe { GetWindow(hwnd, GW_HWNDPREV) };
        let mut checked = 0;
        while !above.is_null() {
            if checked > 4096 {
                return Err("Cannot establish a stable window order".into());
            }
            checked += 1;
            if unsafe { IsWindowVisible(above) } != 0 && unsafe { IsIconic(above) } == 0 {
                let other = rectangle(above)?;
                let mut overlap: RECT = unsafe { zeroed() };
                if unsafe { IntersectRect(&mut overlap, &canvas, &other) } != 0 {
                    return Ok(false);
                }
            }
            above = unsafe { GetWindow(above, GW_HWNDPREV) };
        }
        Ok(true)
    }
    fn pump_observer_messages() -> Result<(), String> {
        let mut message: MSG = unsafe { zeroed() };
        while unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) } != 0 {
            if message.message == WM_QUIT {
                return Err("Observer was closed; trial excluded".into());
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        Ok(())
    }
    struct Reference {
        capture: Capture,
        pixels: Vec<u8>,
        hash: u64,
        list_rect: RECT,
        parent_rect: RECT,
        count: usize,
        before_list_hash: u64,
        cursor: POINT,
    }
    struct Target {
        pid: u32,
        hwnd: HWND,
        edit: HWND,
        list: HWND,
        legacy: bool,
        saved_query: String,
        references: Vec<Reference>,
    }
    impl Target {
        fn new(pid: u32, legacy: bool) -> Result<Self, String> {
            let hwnd = find_target(pid, legacy)?;
            let (edit, list) = children(hwnd)?;
            if unsafe { IsWindowVisible(hwnd) } != 0 {
                return Err("Start with both launchers hidden; no visible input is altered".into());
            }
            Ok(Self {
                pid,
                hwnd,
                edit,
                list,
                legacy,
                saved_query: get_query(edit)?,
                references: Vec::new(),
            })
        }
        fn verify(&self) -> Result<(), String> {
            let expected = if self.legacy {
                "TALTRunForm"
            } else {
                "ALTRun.Native.PerformanceProbe"
            };
            let mut pid = 0;
            unsafe {
                GetWindowThreadProcessId(self.hwnd, &mut pid);
            }
            if pid != self.pid || class(self.hwnd) != expected {
                return Err("Verified target disappeared or changed".into());
            }
            for child in [self.edit, self.list] {
                let mut p = 0;
                unsafe {
                    GetWindowThreadProcessId(child, &mut p);
                }
                if p != self.pid || unsafe { IsChild(self.hwnd, child) } == 0 {
                    return Err("Verified input/results control changed".into());
                }
            }
            Ok(())
        }
        fn active(&self) -> Result<(), String> {
            self.verify()?;
            if unsafe { IsWindowVisible(self.hwnd) } == 0
                || unsafe { GetForegroundWindow() } != self.hwnd
                || foreground_pid() != self.pid
            {
                return Err("Target hidden or foreground changed; observation rejected".into());
            }
            Ok(())
        }
        fn query(&self, q: &str) -> Result<(), String> {
            self.verify()?;
            set_query(self.edit, q)
        }
        fn hide(&self) -> Result<(), String> {
            self.verify()?;
            self.query("")?;
            if unsafe { IsWindowVisible(self.hwnd) } == 0 {
                return Ok(());
            }
            // Direct, bounded messages cannot deliver Escape to another app.
            message(self.edit, WM_KEYDOWN, VK_ESCAPE as usize, 1)?;
            message(
                self.edit,
                WM_KEYUP,
                VK_ESCAPE as usize,
                0xc0000001_u32 as isize,
            )?;
            wait_visible(self.hwnd, false)
        }
        fn wake(&self) -> Result<(), String> {
            self.verify()?;
            wake(self.legacy)?;
            wait_visible(self.hwnd, true)?;
            self.active()
        }
        fn prepare(&mut self) -> Result<(), String> {
            self.wake()?;
            for (index, query) in QUERIES.iter().enumerate() {
                if index > 0 {
                    self.query(query)?;
                }
                thread::sleep(Duration::from_millis(100));
                self.active()?;
                let count = message(self.list, LB_GETCOUNT, 0, 0)?;
                if count < 2 {
                    return Err("Each fixed query needs at least two results".into());
                }
                let list_rect = rectangle(self.list)?;
                let parent_rect = rectangle(self.hwnd)?;
                // Include CMD and the final lower border as a layout witness,
                // but exclude the blinking edit caret and outside desktop.
                let canvas = RECT {
                    left: parent_rect.left,
                    top: list_rect.top,
                    right: parent_rect.right,
                    bottom: parent_rect.bottom,
                };
                let mut capture = Capture::new(canvas)?;
                let hash = capture.hash()?;
                let before_list_hash = capture.part_hash(list_rect)?;
                capture.calibrate_backend()?;
                let pixels = capture.pixels.clone();
                self.references.push(Reference {
                    capture,
                    pixels,
                    hash,
                    list_rect,
                    parent_rect,
                    count,
                    before_list_hash,
                    cursor: POINT { x: 0, y: 0 },
                });
            }
            self.hide()
        }
        fn pre_transition(&mut self, index: usize) -> Result<(), String> {
            self.verify()?;
            let reference = &mut self.references[index];
            if unsafe { GetCursorPos(&mut reference.cursor) } == 0 {
                return Err("Cannot record the starting cursor position".into());
            }
            reference.hash = reference.capture.hash()?;
            reference.before_list_hash = reference.capture.part_hash(reference.list_rect)?;
            reference.pixels.clone_from(&reference.capture.pixels);
            reference.capture.prime_native()?;
            Ok(())
        }
        fn verify_cursor(&self, index: usize) -> Result<(), String> {
            let mut current: POINT = unsafe { zeroed() };
            if unsafe { GetCursorPos(&mut current) } == 0 {
                return Err("Cannot verify cursor stability".into());
            }
            let before = self.references[index].cursor;
            if current.x != before.x || current.y != before.y {
                return Err("Cursor moved during the trial; observation excluded".into());
            }
            Ok(())
        }
        fn input_ready(&self) -> bool {
            let tid = unsafe { GetWindowThreadProcessId(self.hwnd, null_mut()) };
            let mut info: GUITHREADINFO = unsafe { zeroed() };
            info.cbSize = size_of::<GUITHREADINFO>() as u32;
            (unsafe { GetGUIThreadInfo(tid, &mut info) }) != 0 && info.hwndFocus == self.edit
        }
        fn observe_desktop(
            &mut self, index: usize, action: i64, frequency: i64,
            frames: &mut Vec<DesktopObservation>,
        ) -> Result<DesktopBounds, String> {
            frames.clear();
            let mut image_ns = None;
            let mut hash = self.references[index].hash;
            let mut list_hash = self.references[index].before_list_hash;
            loop {
                pump_observer_messages()?;
                self.verify()?;
                self.verify_cursor(index)?;
                let began_ns = ns_since(action, qpc()?, frequency)?;
                let list_rect = rectangle(self.list)?;
                let parent_rect = rectangle(self.hwnd)?;
                let before_ready = unsafe { IsWindowVisible(self.hwnd) } != 0
                    && unsafe { GetForegroundWindow() } == self.hwnd && self.input_ready();
                let canvas = self.references[index].capture.rect;
                let clear_before = before_ready && unoccluded(self.hwnd, canvas)?;
                let reference = &mut self.references[index];
                let mut fresh = false;
                if let NativePoll::Frame(timing) = reference.capture.poll_native(1)? {
                    if timing.present_qpc < action && image_ns.is_none() {
                        // Draining can race a final pre-action image. It is not a response.
                    } else {
                        let stamp = timing.stamp_after(action, frequency)
                            .map_err(|e| format!("Desktop frame rejected: {e:?}; metadata {timing:?}"))?;
                        image_ns = Some(stamp.elapsed_ns);
                        hash = reference.capture.hash_pixels()?;
                        list_hash = reference.capture.part_hash(reference.list_rect)?;
                        fresh = true;
                    }
                }
                let expected_list = reference.list_rect;
                let expected_parent = reference.parent_rect;
                let ready = before_ready && same_rect(list_rect, expected_list)
                    && same_rect(parent_rect, expected_parent)
                    && same_rect(rectangle(self.list)?, list_rect)
                    && same_rect(rectangle(self.hwnd)?, parent_rect)
                    && unsafe { GetForegroundWindow() } == self.hwnd && self.input_ready();
                let clear = ready && clear_before && unoccluded(self.hwnd, canvas)?;
                let ended_ns = ns_since(action, qpc()?, frequency)?;
                frames.push(DesktopObservation {
                    observed: Frame { began_ns, ended_ns, hash, list_hash, ready, unoccluded: clear },
                    image_ns, fresh, accumulated_frames: u32::from(image_ns.is_some()), protected_content: false,
                });
                if ended_ns >= 200_000_000 {
                    let reference = &self.references[index];
                    match desktop_observation::analyze(reference.hash, reference.before_list_hash, frames, 50_000_000, 50_000_000) {
                        Ok(bounds) => {
                            self.active()?;
                            self.verify_cursor(index)?;
                            if get_query(self.edit)? != QUERIES[index]
                                || message(self.list, LB_GETCOUNT, 0, 0)? != reference.count
                                || message(self.list, LB_GETCURSEL, 0, 0)? != 0 {
                                return Err("Logical query/list state changed; content is unverified".into());
                            }
                            return Ok(bounds);
                        }
                        Err(error) => {
                            let retryable = matches!(error, desktop_observation::Rejected::Pixels(
                                Rejected::NotSettled | Rejected::NoVisibleTransition | Rejected::NoListTransition));
                            if !retryable || ended_ns >= 2_000_000_000 {
                                return Err(format!("Desktop observation rejected: {error:?}; samples={}", frames.len()));
                            }
                        }
                    }
                }
                if frames.len() >= 4096 { return Err("Desktop trace limit reached".into()); }
            }
        }
        fn observe(
            &mut self,
            index: usize,
            start: Instant,
            frames: &mut Vec<Frame>,
        ) -> Result<Bounds, String> {
            frames.clear();
            loop {
                pump_observer_messages()?;
                self.verify()?;
                self.verify_cursor(index)?;
                let began_ns = start.elapsed().as_nanos();
                let list_rect = rectangle(self.list)?;
                let parent_rect = rectangle(self.hwnd)?;
                let before_ready = unsafe { IsWindowVisible(self.hwnd) } != 0
                    && unsafe { GetForegroundWindow() } == self.hwnd
                    && self.input_ready();
                let canvas = self.references[index].capture.rect;
                let clear_before = if before_ready {
                    unoccluded(self.hwnd, canvas)?
                } else {
                    false
                };
                let reference = &mut self.references[index];
                let hash = reference.capture.hash()?;
                let list_hash = reference.capture.part_hash(reference.list_rect)?;
                let canvas = reference.capture.rect;
                let expected_list = reference.list_rect;
                let expected_parent = reference.parent_rect;
                let ready = before_ready
                    && same_rect(list_rect, expected_list)
                    && same_rect(parent_rect, expected_parent)
                    && same_rect(rectangle(self.list)?, list_rect)
                    && same_rect(rectangle(self.hwnd)?, parent_rect)
                    && unsafe { GetForegroundWindow() } == self.hwnd
                    && self.input_ready();
                let clear = if ready {
                    clear_before && unoccluded(self.hwnd, canvas)?
                } else {
                    false
                };
                frames.push(Frame {
                    began_ns,
                    ended_ns: start.elapsed().as_nanos(),
                    hash,
                    list_hash,
                    ready,
                    unoccluded: clear,
                });
                if start.elapsed() >= Duration::from_millis(200) {
                    let reference = &self.references[index];
                    let result = settled_bounds(
                        reference.hash,
                        reference.before_list_hash,
                        frames,
                        50_000_000,
                        50_000_000,
                    );
                    match result {
                        Ok(bounds) => {
                            self.active()?;
                            self.verify_cursor(index)?;
                            if get_query(self.edit)? != QUERIES[index]
                                || message(self.list, LB_GETCOUNT, 0, 0)? != reference.count
                                || message(self.list, LB_GETCURSEL, 0, 0)? != 0
                            {
                                return Err(
                                    "Logical query/list state changed; pixels are unverified"
                                        .into(),
                                );
                            }
                            return Ok(bounds);
                        }
                        Err(error) => {
                            let fatal = matches!(
                                error,
                                Rejected::InvalidTimeline
                                    | Rejected::Interrupted
                                    | Rejected::Occluded
                                    | Rejected::SparseSampling
                            );
                            if fatal
                                || start.elapsed() >= Duration::from_secs(2)
                                || frames.len() >= 4096
                            {
                                let difference = pixel_diff(
                                    &reference.pixels,
                                    &reference.capture.pixels,
                                    reference.capture.rect,
                                    reference.list_rect,
                                    rectangle(self.hwnd)?,
                                );
                                return Err(format!(
                                    "Pixel observation rejected: {error:?}; samples={}; difference={difference:?}",
                                    frames.len()
                                ));
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_micros(100));
            }
        }
    }

    impl Drop for Target {
        fn drop(&mut self) {
            if self.verify().is_ok() {
                let _ = self.hide();
                let _ = self.query(&self.saved_query);
            }
        }
    }

    pub fn run(args: Vec<String>) -> Result<(), String> {
        let replay = args.iter().any(|a| a == "--probe-only-replay");
        let value = |name: &str| {
            args.iter()
                .position(|a| a == name)
                .and_then(|i| args.get(i + 1))
                .cloned()
                .ok_or(format!("Missing {name}"))
        };
        if args.iter().any(|a| a == "--inspect-window-stack") {
            let output = std::path::PathBuf::from(value("--output")?);
            if !output.is_absolute()
                || !output
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .starts_with("d:\\")
            {
                return Err("Output must stay on D:".into());
            }
            return inspect_window_stack(output);
        }
        if !replay
            && !args
                .iter()
                .any(|a| a == "--confirm-original-auto-execute-off")
        {
            return Err(
                "First verify original ExecuteIfOnlyOne=0; measurement never sends Enter".into(),
            );
        }
        let original: Option<u32> = if replay {
            None
        } else {
            Some(
                value("--original-pid")?
                    .parse()
                    .map_err(|_| "Invalid original PID")?,
            )
        };
        let probe: u32 = value("--probe-pid")?
            .parse()
            .map_err(|_| "Invalid probe PID")?;
        let output = std::path::PathBuf::from(value("--output")?);
        if !output.is_absolute()
            || !output
                .to_string_lossy()
                .to_ascii_lowercase()
                .starts_with("d:\\")
        {
            return Err("Output must stay on D:".into());
        }
        let rounds: usize = value("--rounds")
            .unwrap_or("50".into())
            .parse()
            .map_err(|_| "Invalid rounds")?;
        if !(1..=1000).contains(&rounds) {
            return Err("Rounds must be between 1 and 1000".into());
        }
        let mut report = Report::new(output);
        if replay {
            report.kind = "observer-self-replay";
            report.stage = "probe_identity";
            report.target = "probe";
        }
        let dxgi = args.iter().any(|a| a == "--dxgi-observer");
        if dxgi {
            report.kind = if replay { "desktop-frame-self-replay" } else { "desktop-frame-settling-observations" };
            report.stage = "capture_init";
            report.clock_hz = qpc_frequency()?;
            let bridge = Bridge::load_adjacent()?;
            CAPTURE_BRIDGE.with(|cell| *cell.borrow_mut() = Some(bridge));
            report.stage = if replay { "probe_identity" } else { "original_identity" };
        }
        if args
            .iter()
            .any(|a| a == "--diagnostic-fixture-fail-after-one")
        {
            #[cfg(debug_assertions)]
            {
                report.kind = "synthetic-test";
                report.stage = "fixture_failure";
                report.target = "probe";
                report.samples.push(
                    "{\"target\":\"probe\",\"metric\":\"synthetic-fixture\",\"lower_ns\":1,\"upper_ns\":2}".into(),
                );
                return Err("Intentional diagnostic fixture failure".into());
            }
            #[cfg(not(debug_assertions))]
            return Err("Synthetic fault injection is unavailable in release builds".into());
        }
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        let mut targets = Vec::new();
        if let Some(pid) = original {
            targets.push(Target::new(pid, true)?);
        }
        report.stage = "probe_identity";
        report.target = "probe";
        targets.push(Target::new(probe, false)?);
        report.stage = "backdrop";
        // Opaque, non-topmost backdrop avoids reference drift from unrelated
        // windows behind the original's translucent skin. Always removed.
        let mut root_rect: RECT = unsafe { zeroed() };
        unsafe {
            GetWindowRect(targets[0].hwnd, &mut root_rect);
        }
        let scene = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                wide("STATIC").as_ptr(),
                wide("").as_ptr(),
                WS_POPUP | WS_BORDER,
                root_rect.left - 16,
                root_rect.top - 16,
                1800,
                1200,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            )
        };
        if scene.is_null() {
            return Err("Cannot create stable measurement backdrop".into());
        }
        struct Scene(HWND);
        impl Drop for Scene {
            fn drop(&mut self) {
                unsafe {
                    DestroyWindow(self.0);
                }
            }
        }
        let _scene = Scene(scene);
        unsafe {
            ShowWindow(scene, SW_SHOWNOACTIVATE);
            UpdateWindow(scene);
        }
        // Prewarm without counting preparation as a measurement.
        for target in &mut targets {
            let label = if target.legacy { "original" } else { "probe" };
            report.stage = "prepare";
            report.target = label;
            target.prepare().map_err(|e| format!("{label}: {e}"))?;
        }
        for iteration in 0..rounds {
            for offset in 0..targets.len() {
                let index = (iteration + offset) % targets.len();
                let target = &mut targets[index];
                let label = if target.legacy { "original" } else { "probe" };
                report.target = label;
                report.iteration = iteration;
                report.query_id = 0;
                report.stage = "wake_pre_transition";
                report.action_qpc = None;
                report.dispatch_qpc = None;
                target.pre_transition(0)?;
                report.stage = "wake_observe";
                target.verify()?;
                if dxgi {
                    report.action_qpc = Some(qpc()?);
                    let dispatched = wake(target.legacy);
                    report.dispatch_qpc = Some(qpc()?);
                    dispatched?;
                    let bounds = target.observe_desktop(0, report.action_qpc.ok_or("Missing action timestamp")?, report.clock_hz, &mut report.current_desktop_trace)?;
                    report.samples.push(desktop_sample_json(&report, "software_hotkey_request_to_desktop_image", &bounds)?);
                    report.current_desktop_trace.clear();
                } else {
                    let start = Instant::now();
                    wake(target.legacy)?;
                    let bounds = target.observe(0, start, &mut report.current_trace)
                        .map_err(|e| format!("{label} iteration={iteration} wake: {e}"))?;
                    report.samples.push(sample_json(label, iteration, 0, "hotkey_to_observed_final_pixels", &bounds, &report.current_trace));
                    report.current_trace.clear();
                }
                for (reference, _) in QUERIES.iter().enumerate().skip(1) {
                    let query = QUERIES[reference];
                    report.query_id = reference;
                    report.stage = "query_pre_transition";
                    report.action_qpc = None;
                    report.dispatch_qpc = None;
                    target.active()?;
                    target.pre_transition(reference)?;
                    report.stage = "query_observe";
                    if dxgi {
                        report.action_qpc = Some(qpc()?);
                        let dispatched = target.query(query);
                        report.dispatch_qpc = Some(qpc()?);
                        dispatched?;
                        let bounds = target.observe_desktop(reference, report.action_qpc.ok_or("Missing action timestamp")?, report.clock_hz, &mut report.current_desktop_trace)?;
                        report.samples.push(desktop_sample_json(&report, "settext_request_to_desktop_image", &bounds)?);
                        report.current_desktop_trace.clear();
                    } else {
                        let start = Instant::now();
                        target.query(query)?;
                        let bounds = target.observe(reference, start, &mut report.current_trace)
                            .map_err(|e| format!("{label} iteration={iteration} query_id={reference}: {e}"))?;
                        report.samples.push(sample_json(label, iteration, reference, "settext_to_observed_final_pixels", &bounds, &report.current_trace));
                        report.current_trace.clear();
                    }
                }
                report.stage = "hide";
                target.hide()?;
            }
        }
        report.complete = true;
        report.stage = "complete";
        report.save()?;
        let count = report.samples.len();
        println!("Captured {count} bounded pixel observations; performance gate remains pending.");
        Ok(())
    }
    #[cfg(test)]
    mod pixel_tests {
        use super::*;

        #[test]
        fn difference_regions_separate_results_from_window_and_desktop() {
            let capture = RECT {
                left: 10,
                top: 20,
                right: 14,
                bottom: 23,
            };
            let list = RECT {
                left: 10,
                top: 20,
                right: 12,
                bottom: 21,
            };
            let parent = RECT {
                left: 10,
                top: 20,
                right: 13,
                bottom: 22,
            };
            let expected = vec![0; 4 * 3 * 4];
            let mut actual = expected.clone();
            actual[0] = 255;
            actual[7] = 1;
            actual[(4 + 2) * 4] = 255;
            actual[(8 + 3) * 4] = 255;
            assert_eq!(
                pixel_diff(&expected, &actual, capture, list, parent),
                PixelDiff {
                    compared: true,
                    list: 2,
                    parent: 1,
                    outside: 1,
                    alpha_only: 1,
                    bounds: Some([10, 20, 14, 23]),
                }
            );
        }
    }
}

fn main() {
    #[cfg(windows)]
    if let Err(message) = local_or_cli() {
        eprintln!("Measurement not accepted: {message}");
        std::process::exit(2);
    }
}

// The desktop tool launches an executable without arguments. This local-only
// sidecar starts the two test processes on the same desktop as the original.
// It neither switches desktops nor installs a service or scheduled task.
#[cfg(windows)]
fn local_or_cli() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return windows::run(args);
    }
    let exe = std::env::current_exe().map_err(|_| "Cannot locate test instrument")?;
    let folder = exe.parent().ok_or("Missing test folder")?;
    if !folder
        .to_string_lossy()
        .to_ascii_lowercase()
        .starts_with("d:\\")
    {
        return Err("Local test session must stay on D:".into());
    }
    let status = folder.join("session-status.txt");
    let result = run_local_session(folder);
    let text = match &result {
        Ok(()) => "Local evidence saved; performance gate remains pending.".to_string(),
        Err(error) => format!("Measurement not accepted: {error}"),
    };
    std::fs::write(status, text).map_err(|_| "Cannot save session status")?;
    result
}

#[cfg(windows)]
fn run_local_session(folder: &std::path::Path) -> Result<(), String> {
    let bytes = std::fs::read(folder.join("altrun-measure.session.txt"))
        .map_err(|_| "No explicit local test session profile")?;
    if bytes.len() > 4096 {
        return Err("Invalid local test profile size".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "Profile must be UTF-8")?;
    let lines: Vec<_> = text.lines().collect();
    if lines.len() != 3 {
        return Err("Local session profile must contain exactly three lines".into());
    }
    if lines[0] == "ALTRUN-OBSERVER-WINDOW-STACK" {
        return windows::run(vec![
            "altrun-measure".into(),
            "--inspect-window-stack".into(),
            "--output".into(),
            folder.join("session-result.json").to_string_lossy().into(),
        ]);
    }
    let dxgi = lines[0] == "ALTRUN-DXGI-PROBE-SELF-REPLAY" || lines[0] == "ALTRUN-DXGI-LOCAL-TEST-AUTOEXEC-OFF";
    let replay = lines[0] == "ALTRUN-PROBE-SELF-REPLAY" || lines[0] == "ALTRUN-DXGI-PROBE-SELF-REPLAY";
    if !replay && lines[0] != "ALTRUN-LOCAL-TEST-AUTOEXEC-OFF" && lines[0] != "ALTRUN-DXGI-LOCAL-TEST-AUTOEXEC-OFF" {
        return Err("Profile must explicitly select replay or confirm auto-execute off".into());
    }
    let original: u32 = lines[1].parse().map_err(|_| "Invalid original PID")?;
    let data = std::path::Path::new(lines[2]);
    if !data.is_absolute() || !lines[2].to_ascii_lowercase().starts_with("d:\\") {
        return Err("Private test data must stay on D:".into());
    }
    struct Probe(std::process::Child);
    impl Drop for Probe {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let child = Probe(
        std::process::Command::new(folder.join("altrun-probe.exe"))
            .arg("--data")
            .arg(data)
            .spawn()
            .map_err(|_| "Cannot start local prototype")?,
    );
    // Startup readiness is outside all measured intervals.
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut args = vec![
        "altrun-measure".into(),
        "--original-pid".into(),
        original.to_string(),
        "--probe-pid".into(),
        child.0.id().to_string(),
        "--confirm-original-auto-execute-off".into(),
        "--output".into(),
        folder.join("session-result.json").to_string_lossy().into(),
        "--rounds".into(),
        "3".into(),
    ];
    if replay {
        args.push("--probe-only-replay".into());
    }
    if dxgi {
        args.push("--dxgi-observer".into());
    }
    windows::run(args)
}
