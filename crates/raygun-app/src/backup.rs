//! Backups (M30): Raygun backups kept in the data root's `backups/` folder, restoring them, and
//! importing a file the user picked (a Raygun backup or a Mihon one, told apart by content).

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use raygun_db::backup::{self, BackupData};
use raygun_model::{BackupFile, BackupOutcome};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::SyncChangedEvent;
use crate::manga::MangaService;
use crate::startup::AppDb;

/// Backups kept; creating one more removes the oldest.
const KEEP: usize = 10;
const EXTENSION: &str = "raygunbk";
/// Bigger files are refused before decompressing (a library backup is far smaller).
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;

pub struct BackupService {
    app: AppHandle,
    dir: PathBuf,
}

impl BackupService {
    pub fn new(app: AppHandle, dir: PathBuf) -> Self {
        Self { app, dir }
    }

    /// Writes a new backup and returns it.
    pub async fn create(&self) -> Result<BackupFile, String> {
        let now = now_ms();
        let data = backup::export(&self.db(), now).await.map_err(to_message)?;
        let json = serde_json::to_vec(&data).map_err(to_message)?;
        let packed = tauri::async_runtime::spawn_blocking(move || gzip(&json))
            .await
            .map_err(to_message)??;
        let name = format!("raygun-{}.{EXTENSION}", stamp(now));
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(to_message)?;
        let path = self.dir.join(&name);
        let partial = self.dir.join(format!("{name}.part"));
        tokio::fs::write(&partial, &packed)
            .await
            .map_err(to_message)?;
        tokio::fs::rename(&partial, &path)
            .await
            .map_err(to_message)?;
        tracing::info!(%name, bytes = packed.len(), "backup created");
        self.prune().await;
        Ok(BackupFile {
            name,
            size_bytes: packed.len() as f64,
            created_at: now as f64,
        })
    }

    /// Backups on this device, newest first.
    pub async fn list(&self) -> Result<Vec<BackupFile>, String> {
        let mut files = Vec::new();
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else {
            return Ok(files);
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_backup_name(&name) {
                continue;
            }
            let Ok(metadata) = entry.metadata().await else {
                continue;
            };
            let created_at = metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0.0, |elapsed| elapsed.as_millis() as f64);
            files.push(BackupFile {
                name,
                size_bytes: metadata.len() as f64,
                created_at,
            });
        }
        // Names carry the time, so they sort by age.
        files.sort_by(|a, b| b.name.cmp(&a.name));
        Ok(files)
    }

    pub async fn restore(&self, name: &str) -> Result<BackupOutcome, String> {
        let path = self.path(name)?;
        let bytes = tokio::fs::read(&path).await.map_err(to_message)?;
        self.import_bytes(bytes).await
    }

    pub async fn delete(&self, name: &str) -> Result<(), String> {
        tokio::fs::remove_file(self.path(name)?)
            .await
            .map_err(to_message)
    }

    /// Restores or imports a file the user picked, sent as base64.
    pub async fn import(&self, base64: &str) -> Result<BackupOutcome, String> {
        if base64.len() > MAX_FILE_BYTES / 3 * 4 + 4 {
            return Err("This file is too large to be a backup".into());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64.trim())
            .map_err(|_| "The file couldn't be read".to_owned())?;
        self.import_bytes(bytes).await
    }

    async fn import_bytes(&self, bytes: Vec<u8>) -> Result<BackupOutcome, String> {
        let unpacked = tauri::async_runtime::spawn_blocking(move || gunzip(&bytes))
            .await
            .map_err(to_message)??;
        let outcome = if unpacked.first() == Some(&b'{') {
            self.restore_raygun(&unpacked).await?
        } else {
            let mihon = crate::mihon::decode(&unpacked)?;
            let sources = self.app.state::<MangaService>().sources().await;
            let summary = crate::mihon::import(&self.db(), &mihon, &sources, now_ms())
                .await
                .map_err(to_message)?;
            tracing::info!(
                titles = summary.titles,
                skipped = summary.skipped.len(),
                "mihon backup imported"
            );
            BackupOutcome::Mihon { summary }
        };
        Ok(outcome)
    }

    async fn restore_raygun(&self, json: &[u8]) -> Result<BackupOutcome, String> {
        let data: BackupData = serde_json::from_slice(json)
            .map_err(|error| format!("This backup is damaged ({error})"))?;
        if data.format != backup::FORMAT {
            return Err("This isn't a Raygun backup".into());
        }
        if data.version > backup::VERSION {
            return Err("This backup is from a newer Raygun; update the app first".into());
        }
        let restored = backup::restore(&self.db(), &data)
            .await
            .map_err(to_message)?;
        tracing::info!(
            received = restored.received,
            changed = restored.changed,
            "backup restored"
        );
        if restored.changed > 0 {
            // Cached settings, collections and history in the UI are stale, as after a sync.
            let _ = SyncChangedEvent(restored.changed as u32).emit(&self.app);
            // Restored settings take effect at once (the theme, sound, content filter…).
            crate::settings::apply_stored(&self.app);
        }
        Ok(BackupOutcome::Raygun {
            received: restored.received as u32,
            changed: restored.changed as u32,
        })
    }

    async fn prune(&self) {
        let Ok(files) = self.list().await else {
            return;
        };
        for old in files.iter().skip(KEEP) {
            let _ = tokio::fs::remove_file(self.dir.join(&old.name)).await;
        }
    }

    fn path(&self, name: &str) -> Result<PathBuf, String> {
        if is_backup_name(name) {
            Ok(self.dir.join(name))
        } else {
            Err("Unknown backup".into())
        }
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }
}

/// `raygun-<digits and dashes>.raygunbk`, so a name never leaves the folder.
fn is_backup_name(name: &str) -> bool {
    name.strip_prefix("raygun-")
        .and_then(|rest| rest.strip_suffix(&format!(".{EXTENSION}")))
        .is_some_and(|stamp| {
            !stamp.is_empty()
                && stamp
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b'-')
        })
}

fn gzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).map_err(to_message)?;
    encoder.finish().map_err(to_message)
}

/// Decompresses a gzip file (bounded, against decompression bombs); other bytes pass through.
fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return Ok(bytes.to_vec());
    }
    let mut unpacked = Vec::new();
    flate2::read::MultiGzDecoder::new(bytes)
        .take(MAX_UNPACKED_BYTES + 1)
        .read_to_end(&mut unpacked)
        .map_err(|_| "The file is damaged".to_owned())?;
    if unpacked.len() as u64 > MAX_UNPACKED_BYTES {
        return Err("This file is too large to be a backup".into());
    }
    Ok(unpacked)
}

/// `yyyymmdd-hhmmss` (UTC) for a unix ms time.
fn stamp(now_ms: i64) -> String {
    let seconds = now_ms.div_euclid(1000);
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_stay_in_the_folder() {
        assert!(is_backup_name("raygun-20261031-174500.raygunbk"));
        assert!(!is_backup_name("raygun-.raygunbk"));
        assert!(!is_backup_name("raygun-../../x.raygunbk"));
        assert!(!is_backup_name("other.raygunbk"));
        assert!(!is_backup_name("raygun-1.raygunbk.part"));
    }

    #[test]
    fn stamps_read_as_utc_dates() {
        assert_eq!(stamp(0), "19700101-000000");
        assert_eq!(stamp(1_793_464_215_000), "20261031-163015");
        assert_eq!(stamp(951_782_400_000), "20000229-000000", "leap day");
    }

    #[test]
    fn gzip_round_trips_and_plain_bytes_pass_through() {
        let packed = gzip(b"{\"format\":1}").unwrap();
        assert_eq!(gunzip(&packed).unwrap(), b"{\"format\":1}");
        assert_eq!(gunzip(b"\x0a\x02hi").unwrap(), b"\x0a\x02hi");
        assert!(gunzip(&[0x1f, 0x8b, 0, 1, 2]).is_err());
    }
}
