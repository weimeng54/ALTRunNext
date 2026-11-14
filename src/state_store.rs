// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! A serialized, coalescing writer. Disk access never happens on input/show.
use crate::usage::{Usage, UsageChange};
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
            return Err(crate::localization::message(
                "ui.file_changed_outside_altrunnext_the_file_was",
                &[("file", &self.path.to_string_lossy())],
            ));
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
pub struct Writer<T = String> {
    sender: Option<mpsc::Sender<WriteRequest<T>>>,
    worker: Option<JoinHandle<()>>,
    pub errors: mpsc::Receiver<String>,
}
enum WriteRequest<T> {
    Update(T),
    Flush(mpsc::Sender<Result<(), String>>),
}
impl Writer<String> {
    pub fn start(file: TextFile) -> Self {
        Self::start_notified(file, || {})
    }
    pub fn start_notified(file: TextFile, notify: impl Fn() + Send + 'static) -> Self {
        Self::start_worker(
            file,
            String::new(),
            |state, text| *state = text,
            |file, text| file.save(text),
            notify,
        )
    }
}

impl Writer<UsageChange> {
    pub fn start_usage(file: TextFile, usage: Usage, notify: impl Fn() + Send + 'static) -> Self {
        Self::start_worker(
            file,
            usage,
            |state, change| change.apply(state),
            |file, usage| file.save(&usage.text()),
            notify,
        )
    }
}

impl<T: Send + 'static> Writer<T> {
    fn start_worker<S: Send + 'static>(
        mut file: TextFile,
        mut state: S,
        apply: fn(&mut S, T),
        save: fn(&mut TextFile, &S) -> Result<(), String>,
        notify: impl Fn() + Send + 'static,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let (errors_tx, errors) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut queued = None;
            let mut previous = Ok(());
            while let Some(request) = queued.take().or_else(|| receiver.recv().ok()) {
                match request {
                    WriteRequest::Update(update) => {
                        apply(&mut state, update);
                        while let Ok(request) = receiver.try_recv() {
                            match request {
                                WriteRequest::Update(newer) => apply(&mut state, newer),
                                flush => {
                                    queued = Some(flush);
                                    break;
                                }
                            }
                        }
                        previous = save(&mut file, &state);
                        if let Err(e) = &previous {
                            let _ = errors_tx.send(e.clone());
                            notify();
                        }
                    }
                    WriteRequest::Flush(reply) => {
                        let _ = reply.send(previous.clone());
                    }
                }
            }
        });
        Self {
            sender: Some(sender),
            worker: Some(worker),
            errors,
        }
    }
    pub fn publish(&self, update: T) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(WriteRequest::Update(update));
        }
    }
    pub fn flush(&self) -> mpsc::Receiver<Result<(), String>> {
        let (reply, result) = mpsc::channel();
        if let Some(sender) = &self.sender {
            let _ = sender.send(WriteRequest::Flush(reply));
        }
        result
    }
}
impl<T> Drop for Writer<T> {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
