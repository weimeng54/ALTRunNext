//! Optional image decoding lives entirely off the resident UI thread.
use std::{ffi::c_void, path::PathBuf, ptr::null_mut};
use windows_sys::Win32::{Foundation::RECT, Graphics::Gdi::*};

#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    path: PathBuf,
    tint: u32,
    strength: i32,
}

impl Request {
    pub fn for_settings(
        prefs: &altrun_probe::preferences::Preferences,
        data: &std::path::Path,
        tint: u32,
    ) -> Option<Self> {
        let path = prefs.get("BGFileName");
        let strength = prefs.number("BGStrength");
        if !prefs.flag("ShowSkin")
            || path.is_empty()
            || strength == 0
            || crate::theme::high_contrast()
        {
            return None;
        }
        Some(Self {
            path: data
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join(path),
            tint,
            strength,
        })
    }
}

pub struct Bitmap {
    handle: usize,
    width: i32,
    height: i32,
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.handle as _) };
    }
}

fn cover(source: (i32, i32), destination: (i32, i32), position: &str) -> (i32, i32, i32, i32) {
    let (w, h) = source;
    let (dw, dh) = destination;
    if i64::from(w) * i64::from(dh) > i64::from(h) * i64::from(dw) {
        let width = (i64::from(h) * i64::from(dw) / i64::from(dh)).max(1) as i32;
        ((w - width) / 2, 0, width, h)
    } else {
        let height = (i64::from(w) * i64::from(dh) / i64::from(dw)).max(1) as i32;
        let y = match position {
            "top" => 0,
            "bottom" => h - height,
            _ => (h - height) / 2,
        };
        (0, y, w, height)
    }
}

impl Bitmap {
    pub fn draw(&self, dc: HDC, rect: RECT, position: &str) {
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return;
        }
        let (x, y, w, h) = cover((self.width, self.height), (width, height), position);
        unsafe {
            let memory = CreateCompatibleDC(dc);
            if memory.is_null() {
                return;
            }
            let old = SelectObject(memory, self.handle as HBITMAP);
            let mode = SetStretchBltMode(dc, HALFTONE);
            StretchBlt(
                dc, rect.left, rect.top, width, height, memory, x, y, w, h, SRCCOPY,
            );
            if mode != 0 {
                SetStretchBltMode(dc, mode);
            }
            SelectObject(memory, old);
            DeleteDC(memory);
        }
    }
}
#[repr(C)]
struct Startup {
    version: u32,
    callback: *const c_void,
    suppress_thread: i32,
    suppress_codecs: i32,
}
#[link(name = "gdiplus")]
unsafe extern "system" {
    fn GdiplusStartup(token: *mut usize, input: *const Startup, output: *mut c_void) -> i32;
    fn GdiplusShutdown(token: usize);
    fn GdipLoadImageFromFile(file: *const u16, image: *mut *mut c_void) -> i32;
    fn GdipCreateHBITMAPFromBitmap(
        image: *mut c_void,
        bitmap: *mut *mut c_void,
        background: u32,
    ) -> i32;
    fn GdipDisposeImage(image: *mut c_void) -> i32;
    fn GdipGetImageWidth(image: *mut c_void, width: *mut u32) -> i32;
    fn GdipGetImageHeight(image: *mut c_void, height: *mut u32) -> i32;
    fn GdipGetImageThumbnail(
        image: *mut c_void,
        width: u32,
        height: u32,
        thumbnail: *mut *mut c_void,
        callback: *const c_void,
        data: *mut c_void,
    ) -> i32;
}
pub fn load(request: &Request) -> Result<Bitmap, String> {
    use std::os::windows::ffi::OsStrExt;
    let wide = request
        .path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    unsafe {
        let mut token = 0;
        let input = Startup {
            version: 1,
            callback: std::ptr::null(),
            suppress_thread: 0,
            suppress_codecs: 0,
        };
        if GdiplusStartup(&mut token, &input, null_mut()) != 0 {
            return Err("无法初始化图片服务".into());
        }
        let mut image = null_mut();
        let mut thumbnail = null_mut();
        let loaded = GdipLoadImageFromFile(wide.as_ptr(), &mut image) == 0;
        let (mut width, mut height) = (0, 0);
        if loaded {
            GdipGetImageWidth(image, &mut width);
            GdipGetImageHeight(image, &mut height);
        }
        let result = if !loaded {
            Err("无法读取背景图片；支持 PNG、JPEG、BMP".into())
        } else if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 25_000_000 {
            Err("背景图片过大，请缩小到 2500 万像素以内".into())
        } else {
            let max = width.max(height).max(1024);
            let w = (width * 1024 / max).max(1);
            let h = (height * 1024 / max).max(1);
            let mut bitmap = null_mut();
            let rgb = request.tint;
            let argb = 0xff000000 | ((rgb & 255) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 255);
            if GdipGetImageThumbnail(image, w, h, &mut thumbnail, std::ptr::null(), null_mut()) != 0
                || GdipCreateHBITMAPFromBitmap(thumbnail, &mut bitmap, argb) != 0
            {
                Err("无法创建背景图片".into())
            } else {
                let bitmap = Bitmap {
                    handle: bitmap as usize,
                    width: w as i32,
                    height: h as i32,
                };
                blend(bitmap, rgb, request.strength)
            }
        };
        if !thumbnail.is_null() {
            GdipDisposeImage(thumbnail);
        }
        if !image.is_null() {
            GdipDisposeImage(image);
        }
        GdiplusShutdown(token);
        result
    }
}

fn blend(bitmap: Bitmap, tint: u32, strength: i32) -> Result<Bitmap, String> {
    let alpha = strength as u32 * 34;
    let mut pixels = vec![0u32; (bitmap.width * bitmap.height) as usize];
    unsafe {
        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            return Err("无法创建背景图片".into());
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = bitmap.width;
        info.bmiHeader.biHeight = bitmap.height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let handle = bitmap.handle as HBITMAP;
        let rows = bitmap.height as u32;
        let ok = GetDIBits(
            dc,
            handle,
            0,
            rows,
            pixels.as_mut_ptr() as _,
            &mut info,
            DIB_RGB_COLORS,
        ) == bitmap.height;
        let ok = ok && {
            for pixel in &mut pixels {
                let mix = |shift: u32| {
                    (((*pixel >> shift) & 255) * alpha
                        + ((tint >> (16 - shift)) & 255) * (10000 - alpha)
                        + 5000)
                        / 10000
                };
                *pixel = mix(16) << 16 | mix(8) << 8 | mix(0);
            }
            SetDIBits(
                dc,
                handle,
                0,
                rows,
                pixels.as_ptr() as _,
                &info,
                DIB_RGB_COLORS,
            ) == bitmap.height
        };
        DeleteDC(dc);
        if ok {
            Ok(bitmap)
        } else {
            Err("无法处理背景图片".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Request, cover, load};

    #[test]
    fn cover_preserves_aspect_and_allows_vertical_alignment() {
        assert_eq!(cover((800, 600), (400, 200), "center"), (0, 100, 800, 400));
        assert_eq!(cover((800, 600), (400, 200), "top"), (0, 0, 800, 400));
        assert_eq!(cover((800, 600), (400, 200), "bottom"), (0, 200, 800, 400));
        assert_eq!(cover((1000, 300), (400, 200), "center"), (200, 0, 600, 300));
    }

    #[test]
    fn decodes_and_tints_a_background_bitmap() {
        let folder = std::env::current_dir().unwrap().join("target/skin-fixture");
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("image.bmp");
        let mut bmp = vec![0u8; 70];
        bmp[..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&70u32.to_le_bytes());
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&2u32.to_le_bytes());
        bmp[22..26].copy_from_slice(&2u32.to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
        bmp[54..].fill(255);
        std::fs::write(&path, bmp).unwrap();
        let image = load(&Request {
            path,
            tint: 0x251e19,
            strength: 68,
        })
        .unwrap();
        assert_eq!((image.width, image.height), (2, 2));
    }
}
