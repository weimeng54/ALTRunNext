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
            let name = format!("ALTRunNext-{hash:016x}");
            Ok(Self {
                registry: r"Software\Microsoft\Windows\CurrentVersion\Run".into(),
                send_to: folder.join("ALTRunNext.lnk"),
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
        let current = self.startup_value(&self.name)?;
        let previous = self
            .previous_name()
            .map(|name| self.startup_value(&name))
            .transpose()?
            .flatten();
        let startup = current.is_some() || previous.is_some();
        let previous_link = self.previous_link().is_some_and(|path| path.is_file());
        let send_to = previous_link || self.link_state()? != LinkState::Other;
        Ok((startup, send_to))
    }

    fn previous_name(&self) -> Option<String> {
        self.name
            .strip_prefix("ALTRunNext-")
            .map(|hash| format!("ALTRunModern-{hash}"))
    }

    fn previous_link(&self) -> Option<PathBuf> {
        self.previous_name()
            .map(|name| self.send_to.with_file_name(format!("{name}.lnk")))
    }

    fn startup_value(&self, name: &str) -> Result<Option<String>, String> {
        unsafe {
            let mut key = null_mut();
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(&self.registry).as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut key,
            );
            if status == ERROR_FILE_NOT_FOUND {
                Ok(None)
            } else if status != 0 {
                return Err(format!("读取启动项失败：{status}"));
            } else {
                let mut b = vec![0u16; 32768];
                let mut size = (b.len() * 2) as u32;
                let mut kind = 0;
                let status = RegQueryValueExW(
                    key,
                    wide(name).as_ptr(),
                    std::ptr::null(),
                    &mut kind,
                    b.as_mut_ptr() as _,
                    &mut size,
                );
                RegCloseKey(key);
                if status == ERROR_FILE_NOT_FOUND {
                    Ok(None)
                } else if status != 0 {
                    return Err(format!("读取启动项失败：{status}"));
                } else {
                    Ok((kind == REG_SZ).then(|| {
                        String::from_utf16_lossy(&b[..(size as usize / 2).min(b.len())])
                            .trim_end_matches('\0')
                            .to_owned()
                    }))
                }
            }
        }
    }
    fn startup(&self, name: &str, value: Option<&str>) -> Result<(), String> {
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
            let status = if let Some(value) = value {
                let command = wide(value);
                RegSetValueExW(
                    key,
                    wide(name).as_ptr(),
                    0,
                    REG_SZ,
                    command.as_ptr() as _,
                    (command.len() * 2) as u32,
                )
            } else {
                RegDeleteValueW(key, wide(name).as_ptr())
            };
            RegCloseKey(key);
            if status != 0 && status != ERROR_FILE_NOT_FOUND {
                return Err(format!("修改启动项失败：{status}"));
            }
            Ok(())
        }
    }
    fn link_arguments(&self) -> String {
        format!("--enable-launch --data \"{}\" --add", self.data.display())
    }

    fn link_state(&self) -> Result<LinkState, String> {
        if !self
            .send_to
            .try_exists()
            .map_err(|error| error.to_string())?
        {
            return Ok(LinkState::Other);
        }
        let script = format!(
            "$ErrorActionPreference='Stop';$w=New-Object -ComObject WScript.Shell;$s=$w.CreateShortcut({});if($s.Arguments -ne {}){{exit 3}};if($s.TargetPath -ne {}){{exit 4}}",
            ps_quote(&self.send_to.to_string_lossy()),
            ps_quote(&self.link_arguments()),
            ps_quote(&self.executable.to_string_lossy()),
        );
        match run_script(&script)?.code() {
            Some(0) => Ok(LinkState::Current),
            Some(3) => Ok(LinkState::Other),
            Some(4) => Ok(LinkState::PreviousExecutable),
            _ => Err("无法读取“发送到”快捷方式".into()),
        }
    }

    fn remove_link(path: &Path) -> Result<(), String> {
        match std::fs::remove_file(path) {
            Ok(()) => {
                notify_send_to(path, SHCNE_DELETE as i32);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn create_link(&self) -> Result<(), String> {
        if let Some(directory) = self.send_to.parent() {
            std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        }
        let script = format!(
            "$ErrorActionPreference='Stop';$w=New-Object -ComObject WScript.Shell;$s=$w.CreateShortcut({});$s.TargetPath={};$s.Arguments={};$s.WorkingDirectory={};$s.Save()",
            ps_quote(&self.send_to.to_string_lossy()),
            ps_quote(&self.executable.to_string_lossy()),
            ps_quote(&self.link_arguments()),
            ps_quote(
                &self
                    .data
                    .parent()
                    .unwrap_or(Path::new("."))
                    .to_string_lossy()
            )
        );
        if run_script(&script)?.success() && self.send_to.is_file() {
            notify_send_to(&self.send_to, SHCNE_CREATE as i32);
            Ok(())
        } else {
            Err("创建“发送到”快捷方式失败".into())
        }
    }

    pub fn apply(&self, startup: bool, send_to: bool) -> Result<(), String> {
        let link = self.link_state()?;
        let previous = self.startup_value(&self.name)?;
        let command = startup.then(|| self.command());
        let old = self.previous_name();
        let old_value = old
            .as_deref()
            .map(|name| self.startup_value(name))
            .transpose()?
            .flatten();
        if previous != command {
            self.startup(&self.name, command.as_deref())?;
        }
        let result = (|| {
            if send_to && link != LinkState::Current {
                self.create_link()?;
            } else if !send_to && link != LinkState::Other {
                Self::remove_link(&self.send_to)?;
            }
            if let Some(path) = self.previous_link() {
                Self::remove_link(&path)?;
            }
            if let Some(name) = &old
                && old_value.is_some()
            {
                self.startup(name, None)?;
            }
            Ok(())
        })();
        if result.is_err() && previous != command {
            let _ = self.startup(&self.name, previous.as_deref());
        }
        result
    }
}

#[derive(PartialEq, Eq)]
enum LinkState {
    Current,
    PreviousExecutable,
    Other,
}

fn notify_send_to(path: &Path, event: i32) {
    let path = wide(&path.to_string_lossy());
    unsafe {
        SHChangeNotify(event, SHCNF_PATHW, path.as_ptr().cast(), std::ptr::null());
    }
}

fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn run_script(script: &str) -> Result<std::process::ExitStatus, String> {
    let bytes = script
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let encoded = base64(&bytes);
    use std::os::windows::process::CommandExt;
    let system = std::env::var_os("SystemRoot").ok_or("缺少 SystemRoot")?;
    std::process::Command::new(
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
    .map_err(|e| e.to_string())
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
