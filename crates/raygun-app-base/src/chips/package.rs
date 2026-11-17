//! A chip's package: downloaded with its size and SHA-256 checked as it arrives, unpacked from
//! `.tar.gz` into a folder beside the chip's, then moved into place. On any failure, a cancel
//! included, nothing is left and an installed copy stays as it was.

use std::io::Read;
use std::path::{Component, Path};

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use super::{INSTALLED, PackageFile};

/// Unpacked packages stay small: a package that grows past this is refused.
const MAX_UNPACKED: u64 = 512 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("unknown chip {0}")]
    Unknown(String),
    #[error("Offline mode is on")]
    Offline,
    #[error("the download failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("couldn't write the chip: {0}")]
    Io(#[from] std::io::Error),
    #[error("the package is damaged (its size or checksum doesn't match)")]
    Mismatch,
    #[error("the package couldn't be unpacked: {0}")]
    Unpack(String),
    #[error("the download was cancelled")]
    Cancelled,
}

pub async fn install(
    client: &reqwest::Client,
    dir: &Path,
    id: &str,
    package: &PackageFile,
    cancel: &CancellationToken,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), PackageError> {
    if raygun_net::network_mode::offline() {
        return Err(PackageError::Offline);
    }
    tokio::fs::create_dir_all(dir).await?;
    let archive = dir.join(format!("{id}.download"));
    let partial = dir.join(format!("{id}.part"));
    let _ = tokio::fs::remove_dir_all(&partial).await;
    let result = async {
        fetch(client, &archive, package, cancel, &mut progress).await?;
        if cancel.is_cancelled() {
            return Err(PackageError::Cancelled);
        }
        let (from, into) = (archive.clone(), partial.clone());
        tokio::task::spawn_blocking(move || unpack(&from, &into))
            .await
            .map_err(|error| PackageError::Unpack(error.to_string()))??;
        tokio::fs::write(partial.join(INSTALLED), &package.sha256).await?;
        if cancel.is_cancelled() {
            return Err(PackageError::Cancelled);
        }
        let target = dir.join(id);
        let _ = tokio::fs::remove_dir_all(&target).await;
        tokio::fs::rename(&partial, &target).await?;
        Ok(())
    }
    .await;
    let _ = tokio::fs::remove_file(&archive).await;
    if result.is_err() {
        let _ = tokio::fs::remove_dir_all(&partial).await;
    }
    result
}

async fn fetch(
    client: &reqwest::Client,
    archive: &Path,
    package: &PackageFile,
    cancel: &CancellationToken,
    progress: &mut impl FnMut(u64, u64),
) -> Result<(), PackageError> {
    progress(0, package.size);
    let mut response = tokio::select! {
        biased;
        () = cancel.cancelled() => return Err(PackageError::Cancelled),
        response = client.get(&package.url).send() => response?.error_for_status()?,
    };
    if response
        .content_length()
        .is_some_and(|length| length > package.size)
    {
        return Err(PackageError::Mismatch);
    }
    let mut out = tokio::fs::File::create(archive).await?;
    let mut hasher = Sha256::new();
    let mut written = 0u64;
    let body = async {
        loop {
            let chunk = tokio::select! {
                biased;
                () = cancel.cancelled() => return Err(PackageError::Cancelled),
                () = raygun_net::network_mode::went_offline() => return Err(PackageError::Offline),
                chunk = response.chunk() => chunk?,
            };
            let Some(chunk) = chunk else {
                return Ok(());
            };
            written += chunk.len() as u64;
            if written > package.size {
                return Err(PackageError::Mismatch);
            }
            hasher.update(&chunk);
            out.write_all(&chunk).await?;
            progress(written, package.size);
        }
    }
    .await;
    // Closed before anything removes it (Windows doesn't delete open files).
    let flushed = out.flush().await;
    drop(out);
    body?;
    flushed?;
    if written != package.size || hex(&hasher.finalize()) != package.sha256 {
        return Err(PackageError::Mismatch);
    }
    Ok(())
}

/// Takes every file out of a `.tar.gz` into `into`. Only plain relative paths are taken; links,
/// `..` and absolute paths are refused.
fn unpack(archive: &Path, into: &Path) -> Result<(), PackageError> {
    let failed = |error: std::io::Error| PackageError::Unpack(error.to_string());
    std::fs::create_dir_all(into)?;
    let reader = std::io::BufReader::new(std::fs::File::open(archive)?);
    let mut entries = tar::Archive::new(flate2::read::GzDecoder::new(reader));
    let mut total = 0u64;
    for entry in entries.entries().map_err(failed)? {
        let mut entry = entry.map_err(failed)?;
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            continue;
        }
        if !kind.is_file() {
            return Err(PackageError::Unpack("it holds a link".into()));
        }
        let path = entry.path().map_err(failed)?.into_owned();
        if !path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
        {
            return Err(PackageError::Unpack(format!(
                "{} leaves the folder",
                path.display()
            )));
        }
        total += entry.size();
        if total > MAX_UNPACKED {
            return Err(PackageError::Unpack("it's too large".into()));
        }
        let out = into.join(&path);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).map_err(failed)?;
        std::fs::write(out, bytes)?;
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
