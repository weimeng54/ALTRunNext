use std::mem::{size_of, zeroed};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};

pub fn save(hwnd: HWND, name: &str) {
    unsafe {
        UpdateWindow(hwnd);
        let mut rect: RECT = zeroed();
        GetWindowRect(hwnd, &mut rect);
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        let source = GetWindowDC(hwnd);
        let memory = CreateCompatibleDC(source);
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut pixels = std::ptr::null_mut();
        let bitmap = CreateDIBSection(
            source,
            &info,
            DIB_RGB_COLORS,
            &mut pixels,
            std::ptr::null_mut(),
            0,
        );
        assert!(!bitmap.is_null() && !pixels.is_null());
        let old = SelectObject(memory, bitmap);
        assert_ne!(
            BitBlt(memory, 0, 0, width, height, source, 0, 0, SRCCOPY),
            0
        );
        GdiFlush();
        let bytes = std::slice::from_raw_parts(pixels as *const u8, (width * height * 4) as usize);
        let mut output = Vec::new();
        output.extend_from_slice(b"BM");
        output.extend_from_slice(&(54 + bytes.len() as u32).to_le_bytes());
        output.extend_from_slice(&[0; 4]);
        output.extend_from_slice(&54_u32.to_le_bytes());
        output.extend_from_slice(&40_u32.to_le_bytes());
        output.extend_from_slice(&width.to_le_bytes());
        output.extend_from_slice(&(-height).to_le_bytes());
        output.extend_from_slice(&1_u16.to_le_bytes());
        output.extend_from_slice(&32_u16.to_le_bytes());
        output.extend_from_slice(&[0; 24]);
        output.extend_from_slice(bytes);
        SelectObject(memory, old);
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(hwnd, source);
        std::fs::create_dir_all("target/ui").unwrap();
        std::fs::write(format!("target/ui/{name}.bmp"), output).unwrap();
    }
}
