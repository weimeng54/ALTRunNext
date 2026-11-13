pub(crate) fn decode_gb18030(bytes: &[u8]) -> Result<String, String> {
    let invalid = || String::from("无法识别快捷项文件的字符编码");
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::{MB_ERR_INVALID_CHARS, MultiByteToWideChar};
        let length = i32::try_from(bytes.len()).map_err(|_| invalid())?;
        let decode = |output, size| unsafe {
            MultiByteToWideChar(
                54936,
                MB_ERR_INVALID_CHARS,
                bytes.as_ptr(),
                length,
                output,
                size,
            )
        };
        let size = decode(std::ptr::null_mut(), 0);
        if size == 0 {
            return Err(invalid());
        }
        let mut output = vec![0; size as usize];
        if decode(output.as_mut_ptr(), size) != size {
            return Err(invalid());
        }
        String::from_utf16(&output).map_err(|_| invalid())
    }
    #[cfg(not(windows))]
    {
        let _ = bytes;
        Err(invalid())
    }
}

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
