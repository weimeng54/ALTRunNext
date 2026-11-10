//! Optional image decoding lives entirely off the resident UI thread.
use std::{ffi::c_void, path::Path, ptr::null_mut};
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
}
pub fn load(path: &Path) -> Result<usize, String> {
    use std::os::windows::ffi::OsStrExt;
    let wide = path
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
        let mut bitmap = null_mut();
        let loaded=GdipLoadImageFromFile(wide.as_ptr(),&mut image)==0;
        let(mut width,mut height)=(0,0);if loaded{GdipGetImageWidth(image,&mut width);GdipGetImageHeight(image,&mut height);}
        let result = if !loaded {
            Err("无法读取背景图片；支持 PNG、JPEG、BMP".into())
        } else if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 4_000_000 {
            Err("背景图片过大，请缩小到四百万像素以内".into())
        } else if GdipCreateHBITMAPFromBitmap(image, &mut bitmap, 0xfff5f7fa) != 0 {
            Err("无法创建背景图片".into())
        } else {
            Ok(bitmap as usize)
        };
        if !image.is_null() {
            GdipDisposeImage(image);
        }
        GdiplusShutdown(token);
        result
    }
}
