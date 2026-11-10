//! Legacy conversion occurs only on import or the execution worker.
pub fn decode_legacy(bytes:&[u8])->Result<String,String>{
    #[cfg(windows)]unsafe{
        use windows_sys::Win32::Globalization::*;
        let n=i32::try_from(bytes.len()).map_err(|_|"文件过大")?;if n==0{return Ok(String::new());}
        let required=MultiByteToWideChar(54936,MB_ERR_INVALID_CHARS,bytes.as_ptr(),n,std::ptr::null_mut(),0);if required==0{return Err("文件既不是有效 UTF-8，也不是 GB18030".into());}
        let mut out=vec![0_u16;required as usize];if MultiByteToWideChar(54936,MB_ERR_INVALID_CHARS,bytes.as_ptr(),n,out.as_mut_ptr(),required)!=required{return Err("旧版文件解码失败".into());}
        String::from_utf16(&out).map_err(|_|"旧版文件包含无效 Unicode".into())
    }
    #[cfg(not(windows))]{let _=bytes;Err("Windows only".into())}
}
pub fn ansi_bytes(text:&str)->Result<Vec<u8>,String>{
    #[cfg(windows)]unsafe{
        use windows_sys::Win32::Globalization::*;
        let page=GetACP();let input:Vec<_>=text.encode_utf16().collect();if input.is_empty(){return Ok(Vec::new());}let n=i32::try_from(input.len()).map_err(|_|"参数过长")?;
        let unicode=page==65001||page==54936;let flags=if unicode{WC_ERR_INVALID_CHARS}else{WC_NO_BEST_FIT_CHARS};let mut changed=0;
        let check=if unicode{std::ptr::null_mut()}else{&mut changed};
        let size=WideCharToMultiByte(page,flags,input.as_ptr(),n,std::ptr::null_mut(),0,std::ptr::null(),check);
        if size==0||changed!=0{return Err("参数不能用旧式 URL 编码表示，请改用 UTF-8 URL 编码".into());}
        let mut out=vec![0_u8;size as usize];
        if WideCharToMultiByte(page,flags,input.as_ptr(),n,out.as_mut_ptr(),size,std::ptr::null(),check)!=size||changed!=0{return Err("旧式 URL 编码失败".into());}Ok(out)
    }
    #[cfg(not(windows))]{let _=text;Err("Windows only".into())}
}
