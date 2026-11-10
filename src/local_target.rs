use std::path::{Path,PathBuf};
pub fn directory(command:&str,base:&Path)->Result<PathBuf,String>{
    let command=command.trim().strip_prefix("@+").or_else(||command.trim().strip_prefix("@-")).or_else(||command.trim().strip_prefix('@')).unwrap_or(command.trim()).trim();
    let target=if let Some(quoted)=command.strip_prefix('"'){let end=quoted.find('"').ok_or("程序路径引号没有闭合")?;format!("\"{}\"",&quoted[..end])}else if Path::new(command).exists(){format!("\"{command}\"")}else{command.split_whitespace().next().unwrap_or("").to_owned()};
    let request=crate::launch::resolve_typed(&target,None,crate::library::ParamType::Auto)?;
    if request.file.contains("://")||request.file.starts_with("shell:")||request.file.starts_with("::"){return Err("网址或系统入口没有本地目录".into());}
    let mut path=PathBuf::from(&request.file);if !path.is_absolute(){if request.file.contains(['\\','/'])||base.join(&path).exists(){path=base.join(path);}else{
        #[cfg(windows)]unsafe{use windows_sys::Win32::Storage::FileSystem::SearchPathW;let file=request.file.encode_utf16().chain(Some(0)).collect::<Vec<_>>();let extension=".exe".encode_utf16().chain(Some(0)).collect::<Vec<_>>();let mut buffer=vec![0u16;32768];let n=SearchPathW(std::ptr::null(),file.as_ptr(),extension.as_ptr(),buffer.len() as u32,buffer.as_mut_ptr(),std::ptr::null_mut());if n>0&&(n as usize)<buffer.len(){path=PathBuf::from(String::from_utf16_lossy(&buffer[..n as usize]));}}
    }}
    let parent=path.parent().filter(|p|!p.as_os_str().is_empty()).ok_or("无法确定命令所在目录")?;Ok(parent.to_path_buf())
}

