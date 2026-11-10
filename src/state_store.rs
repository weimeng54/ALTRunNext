//! A serialized, coalescing writer. Disk access never happens on input/show.
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    thread::JoinHandle,
};
pub struct TextFile {
    pub path: PathBuf,
    original: Option<Vec<u8>>,
}
impl TextFile {
    pub fn open(path: &Path) -> Result<(Self, String), String> {
        let original = match std::fs::read(path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        let text = original
            .as_ref()
            .map(|b| crate::library::decode(b))
            .transpose()?
            .unwrap_or_default();
        Ok((
            Self {
                path: path.into(),
                original,
            },
            text,
        ))
    }
    pub fn save(&mut self, text: &str) -> Result<(), String> {
        use std::io::Write;
        let current = match std::fs::read(&self.path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        if current != self.original {
            return Err(format!("{} 已被外部修改，未覆盖", self.path.display()));
        }
        let stage = self
            .path
            .with_extension(format!("stage-{}", crate::library::new_id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)
            .map_err(|e| e.to_string())?;
        let result = file
            .write_all(text.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string());
        drop(file);
        let result = result
            .and_then(|_| crate::library::replace(&stage, &self.path, self.original.is_some()));
        let _ = std::fs::remove_file(&stage);
        result?;
        self.original = Some(text.as_bytes().to_vec());
        Ok(())
    }
}
pub struct Writer {
    sender: Option<mpsc::Sender<String>>,
    worker: Option<JoinHandle<()>>,
    pub errors: mpsc::Receiver<String>,
}
impl Writer {
    pub fn start(file: TextFile) -> Self {
        Self::start_notified(file, || {})
    }
    pub fn start_notified(mut file: TextFile, notify: impl Fn() + Send + 'static) -> Self {
        let (sender, receiver) = mpsc::channel::<String>();
        let (errors_tx, errors) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            while let Ok(mut text) = receiver.recv() {
                while let Ok(newer) = receiver.try_recv() {
                    text = newer;
                }
                if let Err(e) = file.save(&text) {
                    let _ = errors_tx.send(e);
                    notify();
                }
            }
        });
        Self {
            sender: Some(sender),
            worker: Some(worker),
            errors,
        }
    }
    pub fn publish(&self, text: String) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(text);
        }
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
