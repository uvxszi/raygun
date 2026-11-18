//! Files between linked devices (M46.57): downloaded songs, chapters, episodes and picked covers
//! go over the same encrypted channel as sync, one item and one file at a time.
//!
//! The sender opens with [`Message::Files`], offers its items ([`Message::FilesOffered`]: names
//! and sizes only), and the receiver answers which it wants and how much of each file it already
//! holds from a transfer cut off before ([`Message::FilesWanted`]). Each file then goes as
//! [`Message::FileStart`], raw byte frames, and [`Message::FileEnd`] with the whole file's
//! SHA-256, which the receiver checks. Once an item's files are all in, the receiver's
//! [`FileSink`] keeps it as its own (moves the files where they belong, writes its rows) and says
//! so with [`Message::ItemStored`]; the sender waits for that before the next item. A cancel is
//! [`Message::FilesDone`] at any point: what arrived stays for next time.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use raygun_db::sync::{self, Peer};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::channel::{self, BYTES_PER_FRAME, Channel, Incoming, to_hex};
use crate::protocol::{FileItem, Message, WantedItem};
use crate::{Engine, SyncError, connect};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// An item to send and where its files are on this device (in the order of `item.files`).
#[derive(Debug, Clone)]
pub struct Outgoing {
    pub item: FileItem,
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Sending,
    Receiving,
}

/// Where a transfer is, for progress lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// The other device.
    pub device_id: String,
    pub direction: Direction,
    pub item_id: String,
    pub item_name: String,
    pub kind: String,
    /// The item's place among those going this time (from 0), and how many go.
    pub index: usize,
    pub count: usize,
    /// Bytes moved this time, of all that go.
    pub done_bytes: u64,
    pub total_bytes: u64,
}

pub type ProgressHook = Arc<dyn Fn(&Progress) + Send + Sync>;

/// What one sending session did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SendReport {
    /// Items the receiver didn't have and wanted.
    pub wanted: usize,
    /// Items the receiver kept (or already had: they're not offered again).
    pub delivered: Vec<String>,
    /// Items that didn't go, and why.
    pub failed: Vec<(String, String)>,
    pub cancelled: bool,
}

/// The receiving side, the app's: what it lacks, and keeping what arrived.
pub trait FileSink: Send + Sync {
    /// The folder transfers are written in until they're complete (on the same disk as where
    /// files are kept, so keeping them is a rename).
    fn staging(&self) -> PathBuf;

    /// Which of `items` this device wants (the same length as `items`).
    fn wanted<'a>(&'a self, from: &'a Peer, items: &'a [FileItem]) -> BoxFuture<'a, Vec<bool>>;

    /// Keeps a complete item; `files` are its checked files (in the order of `item.files`),
    /// which the sink moves away. An error goes back to the sender.
    fn store<'a>(
        &'a self,
        from: &'a Peer,
        item: &'a FileItem,
        files: &'a [PathBuf],
    ) -> BoxFuture<'a, Result<(), String>>;

    /// Bytes arrived (called often; the sink throttles what it shows).
    fn progress(&self, _progress: &Progress) {}
}

/// How much of a file is read at a time while hashing a part already sent.
const READ: usize = 1024 * 1024;

impl Engine {
    /// Takes files from paired devices with `sink` (none: they're refused).
    pub fn set_file_sink(&self, sink: Option<Arc<dyn FileSink>>) {
        *self.files.write().expect("files lock poisoned") = sink;
    }

    fn file_sink(&self) -> Option<Arc<dyn FileSink>> {
        self.files.read().expect("files lock poisoned").clone()
    }

    /// Sends `items` to a paired device: the ones it wants, one at a time. `cancel` stops it
    /// between pieces; what arrived there stays for the next time.
    pub async fn send_files(
        &self,
        device_id: &str,
        items: Vec<Outgoing>,
        progress: ProgressHook,
        cancel: Arc<AtomicBool>,
    ) -> Result<SendReport, SyncError> {
        let peer = sync::peer(&self.db, device_id)
            .await?
            .ok_or_else(|| SyncError::Refused("That device isn't paired.".into()))?;
        let address = peer.address.clone().ok_or_else(|| {
            SyncError::Refused(format!("Where {} is isn't known yet.", peer.name))
        })?;
        let stream = connect(&address).await?;
        let (mut channel, remote) = Channel::initiate(stream, &self.identity).await?;
        if to_hex(&remote) != peer.public_key {
            return Err(SyncError::Refused(format!(
                "The device at {address} isn't {} any more. Link it again.",
                peer.name
            )));
        }
        channel
            .send(&Message::Files {
                device_id: self.device_id.clone(),
                name: self.name(),
            })
            .await?;
        match channel.recv().await? {
            Message::Files { device_id, .. } if device_id == peer.device_id => {}
            Message::Refused { reason } => return Err(SyncError::Refused(reason)),
            other => return Err(SyncError::Protocol(format!("{other:?}"))),
        }
        let offered: Vec<FileItem> = items.iter().map(|out| out.item.clone()).collect();
        channel
            .send(&Message::FilesOffered { items: offered })
            .await?;
        let wanted = match channel.recv().await? {
            Message::FilesWanted { wanted } => wanted,
            other => return Err(SyncError::Protocol(format!("{other:?}"))),
        };
        let mut report = SendReport {
            wanted: wanted.len(),
            // What the other device already had counts as delivered.
            delivered: items
                .iter()
                .filter(|out| !wanted.iter().any(|want| want.id == out.item.id))
                .map(|out| out.item.id.clone())
                .collect(),
            ..SendReport::default()
        };
        let plan: Vec<(&Outgoing, &WantedItem)> = wanted
            .iter()
            .filter_map(|want| {
                items
                    .iter()
                    .find(|out| out.item.id == want.id)
                    .map(|out| (out, want))
            })
            .collect();
        let total_bytes: u64 = plan
            .iter()
            .map(|(out, want)| remaining(&out.item, &want.from))
            .sum();
        let mut moved = Moved {
            done: 0,
            total: total_bytes,
            count: plan.len(),
        };
        for (index, (out, want)) in plan.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                report.cancelled = true;
                break;
            }
            if let Some(reason) = changed(out).await {
                channel
                    .send(&Message::ItemSkipped {
                        item: out.item.id.clone(),
                        reason: reason.clone(),
                    })
                    .await?;
                moved.done += remaining(&out.item, &want.from);
                report.failed.push((out.item.id.clone(), reason));
                continue;
            }
            let sent = self
                .send_item(
                    &mut channel,
                    &peer,
                    out,
                    want,
                    (index, &mut moved),
                    &progress,
                    &cancel,
                )
                .await?;
            if !sent {
                report.cancelled = true;
                break;
            }
            match channel.recv().await? {
                Message::ItemStored { item, error: None } => report.delivered.push(item),
                Message::ItemStored {
                    item,
                    error: Some(error),
                } => report.failed.push((item, error)),
                other => return Err(SyncError::Protocol(format!("{other:?}"))),
            }
        }
        channel.send(&Message::FilesDone).await?;
        Ok(report)
    }

    /// One item's files. False when cancelled midway.
    #[allow(clippy::too_many_arguments)]
    async fn send_item<S: channel::Stream>(
        &self,
        channel: &mut Channel<S>,
        peer: &Peer,
        out: &Outgoing,
        want: &WantedItem,
        (index, moved): (usize, &mut Moved),
        progress: &ProgressHook,
        cancel: &AtomicBool,
    ) -> Result<bool, SyncError> {
        let mut buffer = vec![0u8; BYTES_PER_FRAME];
        for (number, (file, path)) in out.item.files.iter().zip(&out.paths).enumerate() {
            let offset = want.from.get(number).copied().unwrap_or(0).min(file.size);
            let mut reader = tokio::fs::File::open(path).await?;
            let mut hasher = Sha256::new();
            // The part already there is hashed, not sent: the hash covers the whole file.
            let mut skipped = 0u64;
            let mut read_buffer = vec![0u8; READ];
            while skipped < offset {
                let want_now = (offset - skipped).min(READ as u64) as usize;
                let read = reader.read(&mut read_buffer[..want_now]).await?;
                if read == 0 {
                    return Err(SyncError::Protocol("a file ended early".into()));
                }
                hasher.update(&read_buffer[..read]);
                skipped += read as u64;
            }
            channel
                .send(&Message::FileStart {
                    item: out.item.id.clone(),
                    file: number as u32,
                    offset,
                })
                .await?;
            let mut left = file.size - offset;
            while left > 0 {
                if cancel.load(Ordering::SeqCst) {
                    return Ok(false);
                }
                let take = left.min(BYTES_PER_FRAME as u64) as usize;
                reader.read_exact(&mut buffer[..take]).await?;
                hasher.update(&buffer[..take]);
                channel.send_bytes(&buffer[..take]).await?;
                left -= take as u64;
                moved.done += take as u64;
                progress(&moved.progress(&peer.device_id, Direction::Sending, &out.item, index));
            }
            channel
                .send(&Message::FileEnd {
                    item: out.item.id.clone(),
                    file: number as u32,
                    sha256: to_hex(&hasher.finalize()),
                })
                .await?;
        }
        Ok(true)
    }

    /// A paired device sends files (after the channel and its key were checked).
    pub(crate) async fn receive_files<S: channel::Stream>(
        &self,
        mut channel: Channel<S>,
        remote: &[u8],
        device_id: &str,
    ) -> Result<(), SyncError> {
        let trusted = sync::peer(&self.db, device_id)
            .await?
            .filter(|peer| peer.public_key == to_hex(remote));
        let (Some(peer), Some(sink)) = (trusted, self.file_sink()) else {
            let reason = "This device doesn't take files from the other one.".to_owned();
            channel
                .send(&Message::Refused {
                    reason: reason.clone(),
                })
                .await?;
            return Err(SyncError::Refused(reason));
        };
        channel
            .send(&Message::Files {
                device_id: self.device_id.clone(),
                name: self.name(),
            })
            .await?;
        let items = match channel.recv().await? {
            Message::FilesOffered { items } => items,
            other => return Err(SyncError::Protocol(format!("{other:?}"))),
        };
        let wants = sink.wanted(&peer, &items).await;
        let staging = sink.staging().join(safe_name(&peer.device_id));
        let mut wanted = Vec::new();
        for (item, _) in items.iter().zip(&wants).filter(|(_, want)| **want) {
            let dir = item_dir(&staging, item);
            let mut from = Vec::with_capacity(item.files.len());
            for (number, file) in item.files.iter().enumerate() {
                let held = tokio::fs::metadata(part_path(&dir, number, file.size))
                    .await
                    .map(|meta| meta.len())
                    .unwrap_or(0);
                from.push(held.min(file.size));
            }
            wanted.push(WantedItem {
                id: item.id.clone(),
                from,
            });
        }
        let total = wanted
            .iter()
            .filter_map(|want| {
                items
                    .iter()
                    .find(|item| item.id == want.id)
                    .map(|item| remaining(item, &want.from))
            })
            .sum();
        let order: Vec<String> = wanted.iter().map(|want| want.id.clone()).collect();
        channel
            .send(&Message::FilesWanted {
                wanted: wanted.clone(),
            })
            .await?;
        let mut moved = Moved {
            done: 0,
            total,
            count: order.len(),
        };
        // Files of the item arriving: whether each checked out.
        let mut checked: Vec<bool> = Vec::new();
        loop {
            match channel.recv().await? {
                Message::FilesDone => return Ok(()),
                Message::ItemSkipped { item, reason } => {
                    tracing::info!(%item, %reason, "a file the other device offered didn't come");
                }
                Message::FileStart { item, file, offset } => {
                    let Some(found) = items.iter().find(|found| found.id == item) else {
                        return Err(SyncError::Protocol(format!("unknown item {item}")));
                    };
                    let index = order.iter().position(|id| *id == item).unwrap_or(0);
                    let number = file as usize;
                    let Some(entry) = found.files.get(number) else {
                        return Err(SyncError::Protocol(format!("unknown file {file}")));
                    };
                    if number == 0 {
                        checked.clear();
                    }
                    let dir = item_dir(&staging, found);
                    let done = self
                        .receive_file(
                            &mut channel,
                            &dir,
                            (number, entry.size, offset),
                            |moved_now| {
                                moved.done += moved_now;
                                sink.progress(&moved.progress(
                                    &peer.device_id,
                                    Direction::Receiving,
                                    found,
                                    index,
                                ));
                            },
                        )
                        .await?;
                    let Some(hasher) = done else {
                        // Cancelled midway: what came stays for next time.
                        return Ok(());
                    };
                    let (end_item, end_file, sha256) = match channel.recv().await? {
                        Message::FileEnd { item, file, sha256 } => (item, file, sha256),
                        Message::FilesDone => return Ok(()),
                        other => return Err(SyncError::Protocol(format!("{other:?}"))),
                    };
                    if end_item != item || end_file != file {
                        return Err(SyncError::Protocol("files out of order".into()));
                    }
                    let good = to_hex(&hasher.finalize()) == sha256;
                    if !good {
                        tracing::warn!(%item, file, "a received file didn't match its checksum");
                        let _ = tokio::fs::remove_file(part_path(&dir, number, entry.size)).await;
                    }
                    checked.push(good);
                    if number + 1 == found.files.len() {
                        let error = self
                            .keep_item(sink.as_ref(), &peer, found, &dir, &checked)
                            .await;
                        channel
                            .send(&Message::ItemStored {
                                item: item.clone(),
                                error,
                            })
                            .await?;
                    }
                }
                other => return Err(SyncError::Protocol(format!("{other:?}"))),
            }
        }
    }

    /// Writes one file's bytes into its part file. Returns the whole file's hasher, or None when
    /// the sender stopped midway.
    async fn receive_file<S: channel::Stream>(
        &self,
        channel: &mut Channel<S>,
        dir: &Path,
        (number, size, offset): (usize, u64, u64),
        mut on_bytes: impl FnMut(u64),
    ) -> Result<Option<Sha256>, SyncError> {
        tokio::fs::create_dir_all(dir).await?;
        let path = part_path(dir, number, size);
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .await?;
        // The sender starts where this device said it was; anything past that goes.
        file.set_len(offset).await?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; READ];
        let mut hashed = 0u64;
        file.seek(std::io::SeekFrom::Start(0)).await?;
        while hashed < offset {
            let take = (offset - hashed).min(READ as u64) as usize;
            let read = file.read(&mut buffer[..take]).await?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            hashed += read as u64;
        }
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        let mut left = size - offset;
        while left > 0 {
            match channel.recv_any().await? {
                Incoming::Bytes(bytes) => {
                    let length = bytes.len() as u64;
                    if length > left {
                        return Err(SyncError::Protocol("a file ran past its size".into()));
                    }
                    file.write_all(&bytes).await?;
                    hasher.update(&bytes);
                    left -= length;
                    on_bytes(length);
                }
                Incoming::Message(Message::FilesDone) => {
                    file.flush().await?;
                    return Ok(None);
                }
                Incoming::Message(other) => {
                    return Err(SyncError::Protocol(format!("{other:?}")));
                }
            }
        }
        file.flush().await?;
        file.sync_all().await?;
        Ok(Some(hasher))
    }

    /// Hands a complete item to the sink; the staging folder goes once it's kept.
    async fn keep_item(
        &self,
        sink: &dyn FileSink,
        peer: &Peer,
        item: &FileItem,
        dir: &Path,
        checked: &[bool],
    ) -> Option<String> {
        if checked.len() != item.files.len() || checked.iter().any(|good| !good) {
            return Some("A file arrived damaged; it goes again next time.".into());
        }
        let files: Vec<PathBuf> = item
            .files
            .iter()
            .enumerate()
            .map(|(number, file)| part_path(dir, number, file.size))
            .collect();
        let result = sink.store(peer, item, &files).await;
        if result.is_ok() {
            let _ = tokio::fs::remove_dir_all(dir).await;
        }
        result.err()
    }
}

struct Moved {
    done: u64,
    total: u64,
    count: usize,
}

impl Moved {
    fn progress(
        &self,
        device_id: &str,
        direction: Direction,
        item: &FileItem,
        index: usize,
    ) -> Progress {
        Progress {
            device_id: device_id.to_owned(),
            direction,
            item_id: item.id.clone(),
            item_name: item.name.clone(),
            kind: item.kind.clone(),
            index,
            count: self.count,
            done_bytes: self.done,
            total_bytes: self.total,
        }
    }
}

/// Bytes of `item` still to go when the receiver holds `from`.
fn remaining(item: &FileItem, from: &[u64]) -> u64 {
    item.files
        .iter()
        .enumerate()
        .map(|(number, file)| file.size - from.get(number).copied().unwrap_or(0).min(file.size))
        .sum()
}

/// Why an item can't go as offered: a file changed size or went.
async fn changed(out: &Outgoing) -> Option<String> {
    if out.paths.len() != out.item.files.len() {
        return Some("its files aren't all here".into());
    }
    for (file, path) in out.item.files.iter().zip(&out.paths) {
        match tokio::fs::metadata(path).await {
            Ok(meta) if meta.len() == file.size => {}
            Ok(_) => return Some(format!("{} changed", file.name)),
            Err(_) => return Some(format!("{} is gone", file.name)),
        }
    }
    None
}

/// Where an item's files wait until they're complete: one folder per item (named by a hash of
/// its id, which may hold any characters).
fn item_dir(staging: &Path, item: &FileItem) -> PathBuf {
    let hash = Sha256::digest(item.id.as_bytes());
    staging.join(&to_hex(&hash)[..24])
}

/// A file's part, named by its place and size: a file whose size changed starts over.
fn part_path(dir: &Path, number: usize, size: u64) -> PathBuf {
    dir.join(format!("{number}-{size}.part"))
}

fn safe_name(text: &str) -> String {
    text.chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || char == '-' {
                char
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
