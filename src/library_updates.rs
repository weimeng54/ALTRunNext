//! A single channel for background reads and explicit saves of shortcut snapshots.
use crate::Entry;
use std::sync::{Arc,atomic::{AtomicU64,Ordering},mpsc::{self,Sender,Receiver}};
struct Update { revision:u64, entries:Vec<Entry> }
#[derive(Clone)]
pub struct LibraryWriter { sender:Sender<Update>, clock:Arc<AtomicU64> }
pub struct LibraryMailbox { receiver:Receiver<Update>, applied:u64 }
pub fn channel()->(LibraryWriter,LibraryMailbox){let(sender,receiver)=mpsc::channel();(LibraryWriter{sender,clock:Arc::new(AtomicU64::new(0))},LibraryMailbox{receiver,applied:0})}
impl LibraryWriter {
    pub fn begin_read(&self)->u64{self.clock.fetch_add(1,Ordering::SeqCst)+1}
    pub fn read_complete(&self,revision:u64,entries:Vec<Entry>){let _=self.sender.send(Update{revision,entries});}
    pub fn saved(&self,entries:Vec<Entry>){self.read_complete(self.begin_read(),entries);}
}
impl LibraryMailbox {
    pub fn take(&mut self)->Option<Vec<Entry>>{let update=self.receiver.try_iter().last()?;self.applied=update.revision;Some(update.entries)}
}
