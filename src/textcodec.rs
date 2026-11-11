pub fn ansi_bytes(text: &str) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Globalization::*;
        let page = GetACP();
        let input: Vec<_> = text.encode_utf16().collect();
        if input.is_empty() {
            return Ok(Vec::new());
        }
        let n = i32::try_from(input.len()).map_err(|_| "参数过长")?;
        let unicode = page == 65001 || page == 54936;
        let flags = if unicode {
            WC_ERR_INVALID_CHARS
        } else {
            WC_NO_BEST_FIT_CHARS
        };
        let mut changed = 0;
        let check = if unicode {
            std::ptr::null_mut()
        } else {
            &mut changed
        };
        let size = WideCharToMultiByte(
            page,
            flags,
            input.as_ptr(),
            n,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            check,
        );
        if size == 0 || changed != 0 {
            return Err("参数不能用Windows ANSI URL 编码表示，请改用 UTF-8 URL 编码".into());
        }
        let mut out = vec![0_u8; size as usize];
        if WideCharToMultiByte(
            page,
            flags,
            input.as_ptr(),
            n,
            out.as_mut_ptr(),
            size,
            std::ptr::null(),
            check,
        ) != size
            || changed != 0
        {
            return Err("Windows ANSI URL 编码失败".into());
        }
        Ok(out)
    }
    #[cfg(not(windows))]
    {
        let _ = text;
        Err("Windows only".into())
    }
}
