use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicUsize;

use tempfile::TempDir;

use super::*;
use crate::protocol::ItemFile;
use crate::tests::{device, pair};

/// Keeps items under `kept/<id>/<name>`, wants what isn't there, and can fail once.
struct TestSink {
    root: PathBuf,
    stored: StdMutex<Vec<String>>,
    progress: AtomicUsize,
}

impl TestSink {
    fn kept(&self, item: &str, name: &str) -> PathBuf {
        self.root.join("kept").join(item).join(name)
    }
}

impl FileSink for TestSink {
    fn staging(&self) -> PathBuf {
        self.root.join("staging")
    }

    fn wanted<'a>(&'a self, _from: &'a Peer, items: &'a [FileItem]) -> BoxFuture<'a, Vec<bool>> {
        Box::pin(async move {
            items
                .iter()
                .map(|item| !self.root.join("kept").join(&item.id).exists())
                .collect()
        })
    }

    fn store<'a>(
        &'a self,
        _from: &'a Peer,
        item: &'a FileItem,
        files: &'a [PathBuf],
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            for (file, path) in item.files.iter().zip(files) {
                let target = self.kept(&item.id, &file.name);
                tokio::fs::create_dir_all(target.parent().unwrap())
                    .await
                    .unwrap();
                tokio::fs::rename(path, target).await.unwrap();
            }
            self.stored.lock().unwrap().push(item.id.clone());
            Ok(())
        })
    }

    fn progress(&self, _progress: &Progress) {
        self.progress.fetch_add(1, Ordering::SeqCst);
    }
}

fn bytes(length: usize, seed: u8) -> Vec<u8> {
    (0..length)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

async fn outgoing(dir: &Path, id: &str, files: &[(&str, Vec<u8>)]) -> Outgoing {
    let mut paths = Vec::new();
    let mut listed = Vec::new();
    for (name, data) in files {
        let path = dir.join(format!("{id}-{name}"));
        tokio::fs::write(&path, data).await.unwrap();
        paths.push(path);
        listed.push(ItemFile {
            name: (*name).into(),
            size: data.len() as u64,
        });
    }
    Outgoing {
        item: FileItem {
            id: id.into(),
            kind: "song".into(),
            name: id.into(),
            meta: serde_json::json!({ "title": id }),
            files: listed,
        },
        paths,
    }
}

fn sink(temp: &TempDir) -> Arc<TestSink> {
    Arc::new(TestSink {
        root: temp.path().join("laptop-files"),
        stored: StdMutex::new(Vec::new()),
        progress: AtomicUsize::new(0),
    })
}

fn quiet() -> ProgressHook {
    Arc::new(|_| {})
}

#[tokio::test]
async fn files_go_to_a_paired_device_once_and_checked() {
    let temp = TempDir::new().unwrap();
    let (phone, laptop) = (device(&temp, "phone").await, device(&temp, "laptop").await);
    pair(&laptop, &phone).await;
    let receiver = sink(&temp);
    laptop.engine.set_file_sink(Some(receiver.clone()));

    let song = bytes(200_000, 1);
    let cover = bytes(10, 2);
    let pages = [bytes(70_000, 3), Vec::new()];
    let items = vec![
        outgoing(temp.path(), "song-a", &[("audio.m4a", song.clone())]).await,
        outgoing(
            temp.path(),
            "chapter-1",
            &[("001.jpg", pages[0].clone()), ("002.jpg", pages[1].clone())],
        )
        .await,
        outgoing(temp.path(), "cover", &[("cover.png", cover.clone())]).await,
    ];
    let seen = Arc::new(StdMutex::new(Vec::new()));
    let watched = seen.clone();
    let report = phone
        .engine
        .send_files(
            laptop.engine.device_id(),
            items.clone(),
            Arc::new(move |progress| watched.lock().unwrap().push(progress.done_bytes)),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert_eq!(report.wanted, 3);
    assert_eq!(report.delivered, ["song-a", "chapter-1", "cover"]);
    assert!(report.failed.is_empty() && !report.cancelled);
    assert_eq!(
        tokio::fs::read(receiver.kept("song-a", "audio.m4a"))
            .await
            .unwrap(),
        song
    );
    assert_eq!(
        tokio::fs::read(receiver.kept("chapter-1", "002.jpg"))
            .await
            .unwrap(),
        pages[1]
    );
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.last(), Some(&(200_000 + 70_000 + 10)));
    assert!(receiver.progress.load(Ordering::SeqCst) > 0);

    // Kept there: nothing goes again, and it counts as delivered.
    let again = phone
        .engine
        .send_files(
            laptop.engine.device_id(),
            items,
            quiet(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert_eq!(again.wanted, 0);
    assert_eq!(again.delivered.len(), 3);
}

#[tokio::test]
async fn a_cancelled_transfer_resumes_where_it_stopped() {
    let temp = TempDir::new().unwrap();
    let (phone, laptop) = (device(&temp, "phone").await, device(&temp, "laptop").await);
    pair(&laptop, &phone).await;
    let cancel = Arc::new(AtomicBool::new(false));
    let receiver = sink(&temp);
    laptop.engine.set_file_sink(Some(receiver.clone()));
    let song = bytes(1_000_000, 9);
    let items = vec![outgoing(temp.path(), "long-song", &[("audio.flac", song.clone())]).await];

    // A few pieces in, the sender is told to stop.
    let stop = cancel.clone();
    let pieces = AtomicUsize::new(0);
    let report = phone
        .engine
        .send_files(
            laptop.engine.device_id(),
            items.clone(),
            Arc::new(move |_| {
                if pieces.fetch_add(1, Ordering::SeqCst) >= 2 {
                    stop.store(true, Ordering::SeqCst);
                }
            }),
            cancel,
        )
        .await
        .unwrap();
    assert!(report.cancelled);
    assert!(report.delivered.is_empty());
    assert!(receiver.stored.lock().unwrap().is_empty());

    // The next time starts from what's there.
    let totals = Arc::new(StdMutex::new(Vec::new()));
    let noted = totals.clone();
    let report = phone
        .engine
        .send_files(
            laptop.engine.device_id(),
            items,
            Arc::new(move |progress| noted.lock().unwrap().push(progress.total_bytes)),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    let total = totals.lock().unwrap()[0];
    assert!(total < song.len() as u64, "only the rest goes: {total}");
    assert_eq!(report.delivered, ["long-song"]);
    assert_eq!(
        tokio::fs::read(receiver.kept("long-song", "audio.flac"))
            .await
            .unwrap(),
        song
    );
}

#[tokio::test]
async fn a_changed_file_is_skipped_and_unpaired_devices_are_refused() {
    let temp = TempDir::new().unwrap();
    let (phone, laptop) = (device(&temp, "phone").await, device(&temp, "laptop").await);
    pair(&laptop, &phone).await;
    let receiver = sink(&temp);
    laptop.engine.set_file_sink(Some(receiver.clone()));
    let item = outgoing(temp.path(), "song-b", &[("audio.mp3", bytes(5_000, 4))]).await;
    // The file grew after it was listed.
    tokio::fs::write(&item.paths[0], bytes(6_000, 4))
        .await
        .unwrap();
    let report = phone
        .engine
        .send_files(
            laptop.engine.device_id(),
            vec![item],
            quiet(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert_eq!(report.failed.len(), 1);
    assert!(report.delivered.is_empty());

    // Without a sink, files are refused.
    laptop.engine.set_file_sink(None);
    let error = phone
        .engine
        .send_files(
            laptop.engine.device_id(),
            Vec::new(),
            quiet(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, SyncError::Refused(_)), "{error}");
}
