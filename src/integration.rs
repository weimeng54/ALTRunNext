//! Per-user startup and File Explorer integration for each data file.
use std::{
    path::{Path, PathBuf},
    ptr::null_mut,
};
use windows_sys::Win32::{Foundation::*, System::Registry::*, UI::Shell::*};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub struct Integration {
    pub registry: String,
    pub name: String,
    pub send_to: PathBuf,
    pub executable: PathBuf,
    pub data: PathBuf,
}
impl Integration {
    pub fn for_data(data: &Path) -> Result<Self, String> {
        let data = std::path::absolute(data).map_err(|e| e.to_string())?;
        unsafe {
            let mut b = vec![0u16; 32768];
            if SHGetFolderPathW(null_mut(), 9, null_mut(), 0, b.as_mut_ptr()) < 0 {
                return Err("无法找到“发送到”目录".into());
            }
            let folder = PathBuf::from(String::from_utf16_lossy(
                &b[..b.iter().position(|c| *c == 0).unwrap_or(0)],
            ));
            let mut hash = 0u64;
            for byte in data.to_string_lossy().to_lowercase().bytes() {
                hash = hash.wrapping_mul(131).wrapping_add(byte as u64);
            }
            let name = format!("ALTRunModern-{hash:016x}");
            Ok(Self {
                registry: r"Software\Microsoft\Windows\CurrentVersion\Run".into(),
                send_to: folder.join(format!("{name}.lnk")),
                name,
                executable: std::env::current_exe().map_err(|e| e.to_string())?,
                data,
            })
        }
    }
    pub fn command(&self) -> String {
        format!(
            "\"{}\" --enable-launch --data \"{}\"",
            self.executable.display(),
            self.data.display()
        )
    }
    pub fn status(&self) -> Result<(bool, bool), String> {
        unsafe {
            let mut key = null_mut();
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(&self.registry).as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut key,
            );
            let startup = if status == ERROR_FILE_NOT_FOUND {
                false
            } else if status != 0 {
                return Err(format!("读取启动项失败：{status}"));
            } else {
                let mut b = vec![0u16; 32768];
                let mut size = (b.len() * 2) as u32;
                let mut kind = 0;
                let status = RegQueryValueExW(
                    key,
                    wide(&self.name).as_ptr(),
                    std::ptr::null(),
                    &mut kind,
                    b.as_mut_ptr() as _,
                    &mut size,
                );
                RegCloseKey(key);
                if status == ERROR_FILE_NOT_FOUND {
                    false
                } else if status != 0 {
                    return Err(format!("读取启动项失败：{status}"));
                } else {
                    kind == REG_SZ
                        && String::from_utf16_lossy(&b[..(size as usize / 2).min(b.len())])
                            .trim_end_matches('\0')
                            == self.command()
                }
            };
            Ok((startup, self.send_to.exists()))
        }
    }
    fn startup(&self, enabled: bool) -> Result<(), String> {
        unsafe {
            let mut key = null_mut();
            let status = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(&self.registry).as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            );
            if status != 0 {
                return Err(format!("无法修改启动项：{status}"));
            }
            let status = if enabled {
                let command = wide(&self.command());
                RegSetValueExW(
                    key,
                    wide(&self.name).as_ptr(),
                    0,
                    REG_SZ,
                    command.as_ptr() as _,
                    (command.len() * 2) as u32,
                )
            } else {
                RegDeleteValueW(key, wide(&self.name).as_ptr())
            };
            RegCloseKey(key);
            if status != 0 && status != ERROR_FILE_NOT_FOUND {
                return Err(format!("修改启动项失败：{status}"));
            }
            Ok(())
        }
    }
    fn shortcut(&self, enabled: bool) -> Result<(), String> {
        if !enabled {
            return match std::fs::remove_file(&self.send_to) {
                Ok(()) => {
                    self.notify_send_to(SHCNE_DELETE);
                    Ok(())
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.to_string()),
            };
        }
        if let Some(directory) = self.send_to.parent() {
            std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        }
        let quote = |s: &str| format!("'{}'", s.replace('\'', "''"));
        let script = format!(
            "$ErrorActionPreference='Stop';$w=New-Object -ComObject WScript.Shell;$s=$w.CreateShortcut({});$s.TargetPath={};$s.Arguments={};$s.WorkingDirectory={};$s.Save()",
            quote(&self.send_to.to_string_lossy()),
            quote(&self.executable.to_string_lossy()),
            quote(&format!(
                "--enable-launch --data \"{}\" --add",
                self.data.display()
            )),
            quote(
                &self
                    .data
                    .parent()
                    .unwrap_or(Path::new("."))
                    .to_string_lossy()
            )
        );
        let bytes = script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let encoded = base64(&bytes);
        use std::os::windows::process::CommandExt;
        let system = std::env::var_os("SystemRoot").ok_or("缺少 SystemRoot")?;
        let status = std::process::Command::new(
            PathBuf::from(system).join(r"System32\WindowsPowerShell\v1.0\powershell.exe"),
        )
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &encoded,
        ])
        .creation_flags(0x08000000)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
        if status.success() && self.send_to.exists() {
            self.notify_send_to(SHCNE_CREATE);
            Ok(())
        } else {
            Err("创建“发送到”快捷方式失败".into())
        }
    }
    fn notify_send_to(&self, event: i32) {
        let path = wide(&self.send_to.to_string_lossy());
        unsafe {
            SHChangeNotify(event, SHCNF_PATHW, path.as_ptr().cast(), std::ptr::null());
        }
    }
    pub fn apply(&self, startup: bool, send_to: bool) -> Result<(), String> {
        let previous = self.status()?;
        if previous.0 != startup {
            self.startup(startup)?;
        }
        if previous.1 != send_to
            && let Err(error) = self.shortcut(send_to)
        {
            if previous.0 != startup {
                let _ = self.startup(previous.0);
            }
            return Err(error);
        }
        Ok(())
    }
}
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as usize;
        let b = chunk.get(1).copied().unwrap_or(0) as usize;
        let c = chunk.get(2).copied().unwrap_or(0) as usize;
        for value in [
            TABLE[a >> 2],
            TABLE[((a & 3) << 4) | (b >> 4)],
            if chunk.len() > 1 {
                TABLE[((b & 15) << 2) | (c >> 6)]
            } else {
                b'='
            },
            if chunk.len() > 2 { TABLE[c & 63] } else { b'=' },
        ] {
            result.push(value as char);
        }
    }
    result
}
