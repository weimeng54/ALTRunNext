//! Bounded child-process execution; never call from the window thread.
use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const MAX_BYTES: usize = 8 * 1024 * 1024;
fn put(out: &mut Vec<u8>, s: &str) -> Result<(), String> {
    if s.len() > MAX_BYTES {
        return Err("匹配数据过大".into());
    }
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
    Ok(())
}
fn number(input: &mut impl Read) -> Result<u32, String> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Ok(u32::from_le_bytes(bytes))
}
fn string(input: &mut impl Read, budget: &mut usize) -> Result<String, String> {
    let n = number(input)? as usize;
    if n > *budget {
        return Err("匹配数据过大".into());
    }
    *budget -= n;
    let mut bytes = vec![0; n];
    input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
pub fn run(
    program: &Path,
    pattern: &str,
    aliases: &[String],
    prefix: bool,
    timeout: Duration,
) -> Result<Vec<Option<usize>>, String> {
    if timeout.is_zero() {
        return Err("正则匹配超时".into());
    }
    if pattern.len() > 4096 || aliases.len() > 100_000 {
        return Err("匹配数据过大".into());
    }
    let mut data = Vec::new();
    put(&mut data, pattern)?;
    data.push(u8::from(prefix));
    data.extend_from_slice(&(aliases.len() as u32).to_le_bytes());
    for alias in aliases {
        put(&mut data, alias)?;
        if data.len() > MAX_BYTES {
            return Err("匹配数据过大".into());
        }
    }
    let mut command = Command::new(program);
    command
        .arg("--regex-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00004000);
    }
    let start = Instant::now();
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let _job = match Job::attach(&child) {
        Ok(j) => j,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
    };
    let mut input = child.stdin.take().ok_or("缺少工作进程输入")?;
    let mut output = child.stdout.take().ok_or("缺少工作进程输出")?;
    let (sender, receiver) = std::sync::mpsc::channel();
    let writer = std::thread::spawn(move || {
        let _ = input.write_all(&data);
    });
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = output
            .by_ref()
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|e| e.to_string());
        let _ = sender.send(result);
    });
    let result = receiver
        .recv_timeout(timeout.saturating_sub(start.elapsed()))
        .map_err(|_| "正则匹配超时".to_owned())
        .and_then(|r| r);
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait().map_err(|e| e.to_string());
    let _ = writer.join();
    let _ = reader.join();
    let bytes = result?;
    if !status?.success() {
        return Err("正则表达式无效或超过运行限制".into());
    }
    if bytes.len() != aliases.len() * 8 {
        return Err("工作进程返回无效数据".into());
    }
    Ok(bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| {
            let value = u64::from_le_bytes(*b);
            if value == u64::MAX {
                None
            } else {
                usize::try_from(value).ok()
            }
        })
        .collect())
}
pub fn worker_main() -> Result<(), String> {
    let mut input = std::io::stdin().lock();
    let mut budget = MAX_BYTES;
    let pattern = string(&mut input, &mut budget)?;
    let mut flag = [0];
    input.read_exact(&mut flag).map_err(|e| e.to_string())?;
    let count = number(&mut input)? as usize;
    if count > 100_000 {
        return Err("匹配数据过大".into());
    }
    let mut aliases = Vec::with_capacity(count);
    for _ in 0..count {
        aliases.push(string(&mut input, &mut budget)?);
    }
    let hits = crate::matching::positions(&pattern, &aliases, flag[0] != 0)?;
    let mut output = std::io::stdout().lock();
    for hit in hits {
        output
            .write_all(&hit.map_or(u64::MAX, |v| v as u64).to_le_bytes())
            .map_err(|e| e.to_string())?;
    }
    output.flush().map_err(|e| e.to_string())
}
#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Job {
    fn attach(child: &std::process::Child) -> Result<Self, String> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{Foundation::*, System::JobObjects::*};
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(format!("无法创建隔离任务：{}", GetLastError()));
            }
            let job = Self(handle);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_PROCESS_MEMORY
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
            info.BasicLimitInformation.ActiveProcessLimit = 1;
            info.ProcessMemoryLimit = 64 * 1024 * 1024;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle() as HANDLE) == 0
            {
                return Err(format!("无法隔离匹配进程：{}", GetLastError()));
            }
            Ok(job)
        }
    }
}
#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
