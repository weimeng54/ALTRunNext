use std::path::{Path, PathBuf};
pub fn directory(command: &str, base: &Path) -> Result<PathBuf, String> {
    let file = crate::launch::resolve_target(command, base)?;
    let lower = file.to_ascii_lowercase();
    if file.contains("://")
        || lower.starts_with("mailto:")
        || lower.starts_with("shell:")
        || file.starts_with("::")
    {
        return Err("ui.web_addresses_and_windows_locations_do_not".into());
    }
    let mut path = PathBuf::from(&file);
    if !path.is_absolute() {
        if file.contains(['\\', '/'])
            || (!crate::launch::is_network_path(&base.to_string_lossy())
                && base.join(&path).exists())
        {
            path = base.join(path);
        } else {
            #[cfg(windows)]
            unsafe {
                use windows_sys::Win32::Storage::FileSystem::SearchPathW;
                let file = file.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
                let extension = ".exe".encode_utf16().chain(Some(0)).collect::<Vec<_>>();
                let mut buffer = vec![0u16; 32768];
                let n = SearchPathW(
                    std::ptr::null(),
                    file.as_ptr(),
                    extension.as_ptr(),
                    buffer.len() as u32,
                    buffer.as_mut_ptr(),
                    std::ptr::null_mut(),
                );
                if n > 0 && (n as usize) < buffer.len() {
                    path = PathBuf::from(String::from_utf16_lossy(&buffer[..n as usize]));
                }
            }
        }
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("ui.could_not_find_the_command_s_folder")?;
    Ok(parent.to_path_buf())
}
