//! The model manager: a built-in catalogue of downloadable models, each installed as a folder
//! `models/<id>/` holding exactly its files. Downloads are size-capped and SHA-256-checked, and
//! land in `<id>.part/` until every file checks out; a cancelled one stops at once and leaves
//! nothing behind.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
pub use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    /// What runs it, where the kind has more than one runner: `anime4k` (shaders the readers run
    /// on the graphics card), `ncnn` (no runner in this build yet).
    #[serde(default)]
    pub engine: Option<String>,
    pub description: String,
    pub license: String,
    pub license_url: String,
    pub source_url: String,
    /// What a translation model takes and writes (M46.48).
    #[serde(default)]
    pub translate: Option<TranslatePair>,
    pub files: Vec<ModelFile>,
}

/// A translation model's languages (M46.48): `from` is `en` or `mul` (any language); it writes
/// each of `to`, and each of `targets` (BCP 47 tag → the `>>code<<` its input starts with).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TranslatePair {
    pub from: Vec<String>,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub targets: std::collections::BTreeMap<String, String>,
}

impl ModelInfo {
    pub fn size(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelFile {
    pub name: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("unknown model {0}")]
    Unknown(String),
    #[error("download failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("couldn't write the model: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0} is larger than expected")]
    TooLarge(String),
    #[error("{0} is damaged (its size or checksum doesn't match)")]
    Mismatch(String),
    #[error("the download was cancelled")]
    Cancelled,
}

/// The built-in catalogue.
pub fn catalog() -> Vec<ModelInfo> {
    serde_json::from_str(include_str!("catalog.json")).expect("the built-in catalogue is valid")
}

pub fn find(id: &str) -> Result<ModelInfo, ModelError> {
    catalog()
        .into_iter()
        .find(|model| model.id == id)
        .ok_or_else(|| ModelError::Unknown(id.to_owned()))
}

/// Folder an installed model lives in.
pub fn model_dir(models_dir: &Path, model: &ModelInfo) -> PathBuf {
    models_dir.join(&model.id)
}

/// Whether every file of the model is present at its exact size.
pub fn is_installed(models_dir: &Path, model: &ModelInfo) -> bool {
    let dir = model_dir(models_dir, model);
    model.files.iter().all(|file| {
        std::fs::metadata(dir.join(&file.name)).is_ok_and(|meta| meta.len() == file.size)
    })
}

/// Downloads every file of `model`, calling `progress(received, total)` as bytes arrive, and
/// installs it. `cancel` stops it wherever it is (waiting for an answer, between two pieces) with
/// [`ModelError::Cancelled`]. On any failure, a cancel included, nothing is left behind and an
/// installed copy stays as it was.
pub async fn download(
    client: &reqwest::Client,
    models_dir: &Path,
    model: &ModelInfo,
    cancel: &CancellationToken,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), ModelError> {
    let target = model_dir(models_dir, model);
    let partial = models_dir.join(format!("{}.part", model.id));
    let _ = tokio::fs::remove_dir_all(&partial).await;
    if cancel.is_cancelled() {
        return Err(ModelError::Cancelled);
    }
    tokio::fs::create_dir_all(&partial).await?;
    let fetched = fetch_all(client, &partial, model, cancel, &mut progress).await;
    // Cancelled after its last byte: still nothing is installed.
    let fetched = fetched.and_then(|()| {
        if cancel.is_cancelled() {
            Err(ModelError::Cancelled)
        } else {
            Ok(())
        }
    });
    if let Err(error) = fetched {
        let _ = tokio::fs::remove_dir_all(&partial).await;
        return Err(error);
    }
    let _ = tokio::fs::remove_dir_all(&target).await;
    tokio::fs::rename(&partial, &target).await?;
    Ok(())
}

async fn fetch_all(
    client: &reqwest::Client,
    dir: &Path,
    model: &ModelInfo,
    cancel: &CancellationToken,
    progress: &mut impl FnMut(u64, u64),
) -> Result<(), ModelError> {
    let total = model.size();
    let mut received = 0u64;
    progress(0, total);
    for file in &model.files {
        // Plain file names only: the catalogue is built in, but stay inside the folder anyway.
        if file.name.contains(['/', '\\']) || file.name.starts_with('.') {
            return Err(ModelError::Mismatch(file.name.clone()));
        }
        let response = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(ModelError::Cancelled),
            response = client.get(&file.url).send() => response?.error_for_status()?,
        };
        if response
            .content_length()
            .is_some_and(|length| length > file.size)
        {
            return Err(ModelError::TooLarge(file.name.clone()));
        }
        let mut out = tokio::fs::File::create(dir.join(&file.name)).await?;
        let written = write_body(response, &mut out, file, cancel, |bytes| {
            received += bytes;
            progress(received, total);
        })
        .await;
        // The file is closed before anything removes it: a write still on its way would keep
        // it open (and Windows doesn't delete open files).
        let flushed = out.flush().await;
        drop(out);
        let (written, hash) = written?;
        flushed?;
        if written != file.size || hash != file.sha256 {
            return Err(ModelError::Mismatch(file.name.clone()));
        }
    }
    Ok(())
}

/// Writes a file's body as it arrives, stopping at a cancel; its length and SHA-256 come back.
async fn write_body(
    response: reqwest::Response,
    out: &mut tokio::fs::File,
    file: &ModelFile,
    cancel: &CancellationToken,
    mut arrived: impl FnMut(u64),
) -> Result<(u64, String), ModelError> {
    let mut hasher = Sha256::new();
    let mut written = 0u64;
    let mut body = response.bytes_stream();
    loop {
        let chunk = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(ModelError::Cancelled),
            chunk = body.next() => chunk,
        };
        let Some(chunk) = chunk else {
            break;
        };
        let chunk = chunk?;
        written += chunk.len() as u64;
        if written > file.size {
            return Err(ModelError::TooLarge(file.name.clone()));
        }
        hasher.update(&chunk);
        out.write_all(&chunk).await?;
        arrived(chunk.len() as u64);
    }
    Ok((written, hex(&hasher.finalize())))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Removes an installed model.
pub async fn delete(models_dir: &Path, model: &ModelInfo) -> Result<(), ModelError> {
    match tokio::fs::remove_dir_all(model_dir(models_dir, model)).await {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::routing::get;

    #[test]
    fn catalogue_is_valid() {
        let models = catalog();
        assert!(models.len() >= 4);
        let mut ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), models.len(), "unique ids");
        for model in &models {
            assert!(model.license_url.starts_with("https://"));
            assert!(model.size() > 0);
            for file in &model.files {
                assert!(file.url.starts_with("https://"), "{}", file.url);
                assert_eq!(file.sha256.len(), 64);
                assert!(file.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            }
        }
        assert!(find("realcugan-up2x-conservative").is_ok());
        assert!(matches!(find("nope"), Err(ModelError::Unknown(_))));
        // Translation models (M46.48) say what they translate and have Marian's files.
        let translators: Vec<&ModelInfo> = models
            .iter()
            .filter(|model| model.kind == "translate")
            .collect();
        assert!(translators.len() >= 2);
        for model in translators {
            let pair = model.translate.as_ref().expect("a translation pair");
            assert!(!pair.from.is_empty());
            assert!(
                !pair.to.is_empty() || !pair.targets.is_empty(),
                "{}",
                model.id
            );
            for name in [
                "config.json",
                "vocab.json",
                "source.spm",
                "model.safetensors",
            ] {
                assert!(
                    model.files.iter().any(|file| file.name == name),
                    "{} lacks {name}",
                    model.id
                );
            }
        }
    }

    fn sha(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }

    async fn serve() -> String {
        let app = Router::new()
            .route("/a.param", get(|| async { "param-data" }))
            .route("/a.bin", get(|| async { vec![7u8; 5000] }))
            // A piece of the file, then nothing more for as long as the client waits.
            .route(
                "/slow.bin",
                get(|| async {
                    let first = futures_util::stream::once(async {
                        Ok::<_, std::io::Error>(axum::body::Bytes::from(vec![7u8; 1000]))
                    });
                    axum::body::Body::from_stream(first.chain(futures_util::stream::pending()))
                }),
            )
            // No answer at all.
            .route(
                "/hang.bin",
                get(|| async {
                    std::future::pending::<()>().await;
                    "never"
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{address}")
    }

    fn model(base: &str, bin_hash: String, bin_size: u64) -> ModelInfo {
        ModelInfo {
            id: "test-model".into(),
            name: "Test".into(),
            kind: "upscale".into(),
            engine: None,
            description: String::new(),
            license: "MIT".into(),
            license_url: "https://example.com".into(),
            source_url: "https://example.com".into(),
            translate: None,
            files: vec![
                ModelFile {
                    name: "a.param".into(),
                    url: format!("{base}/a.param"),
                    sha256: sha(b"param-data"),
                    size: 10,
                },
                ModelFile {
                    name: "a.bin".into(),
                    url: format!("{base}/a.bin"),
                    sha256: bin_hash,
                    size: bin_size,
                },
            ],
        }
    }

    #[tokio::test]
    async fn downloads_checks_and_deletes() {
        let base = serve().await;
        let dir = tempfile::tempdir().unwrap();
        let client = raygun_net::http_client().unwrap();
        let go = CancellationToken::new();

        let good = model(&base, sha(&[7u8; 5000]), 5000);
        let mut seen = Vec::new();
        download(&client, dir.path(), &good, &go, |received, total| {
            seen.push((received, total))
        })
        .await
        .unwrap();
        assert!(is_installed(dir.path(), &good));
        assert_eq!(seen.first(), Some(&(0, 5010)));
        assert_eq!(seen.last(), Some(&(5010, 5010)));
        assert!(!dir.path().join("test-model.part").exists());

        delete(dir.path(), &good).await.unwrap();
        assert!(!is_installed(dir.path(), &good));
        delete(dir.path(), &good).await.unwrap();

        let wrong_hash = model(&base, sha(b"other"), 5000);
        let error = download(&client, dir.path(), &wrong_hash, &go, |_, _| {}).await;
        assert!(matches!(error, Err(ModelError::Mismatch(_))), "{error:?}");
        let too_long = model(&base, sha(&[7u8; 5000]), 4000);
        let error = download(&client, dir.path(), &too_long, &go, |_, _| {}).await;
        assert!(matches!(error, Err(ModelError::TooLarge(_))), "{error:?}");
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            0,
            "nothing left behind"
        );
    }

    /// `model`'s second file swapped for one served from `path`.
    fn with_second(base: &str, path: &str) -> ModelInfo {
        let mut model = model(base, sha(&[7u8; 5000]), 5000);
        model.files[1].url = format!("{base}{path}");
        model
    }

    #[tokio::test]
    async fn a_cancel_stops_the_transfer_and_keeps_nothing_of_it() {
        let base = serve().await;
        let dir = tempfile::tempdir().unwrap();
        let client = raygun_net::http_client().unwrap();

        // Already installed: a cancelled download of it again leaves this copy alone.
        let good = model(&base, sha(&[7u8; 5000]), 5000);
        download(
            &client,
            dir.path(),
            &good,
            &CancellationToken::new(),
            |_, _| {},
        )
        .await
        .unwrap();

        // Cancelled once part of the second file is in, while the server holds the rest back.
        let slow = with_second(&base, "/slow.bin");
        let cancel = CancellationToken::new();
        let mut last = 0;
        let stopped = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            download(&client, dir.path(), &slow, &cancel, |received, _| {
                last = received;
                if received >= 1010 {
                    cancel.cancel();
                }
            }),
        )
        .await
        .expect("a cancel doesn't wait for the server");
        assert!(matches!(stopped, Err(ModelError::Cancelled)), "{stopped:?}");
        assert_eq!(last, 1010);
        assert!(
            !dir.path().join("test-model.part").exists(),
            "no partial files"
        );
        assert!(is_installed(dir.path(), &good), "the installed copy stays");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn a_cancel_stops_a_download_still_waiting_for_its_answer() {
        let base = serve().await;
        let dir = tempfile::tempdir().unwrap();
        let client = raygun_net::http_client().unwrap();
        let hanging = with_second(&base, "/hang.bin");
        let cancel = CancellationToken::new();
        let later = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            later.cancel();
        });
        let stopped = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            download(&client, dir.path(), &hanging, &cancel, |_, _| {}),
        )
        .await
        .expect("a cancel doesn't wait for the answer");
        assert!(matches!(stopped, Err(ModelError::Cancelled)), "{stopped:?}");
        assert!(!is_installed(dir.path(), &hanging));
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            0,
            "nothing left behind"
        );

        // Cancelled before it starts: nothing is fetched.
        let error = download(&client, dir.path(), &hanging, &cancel, |_, _| {
            panic!("nothing arrives")
        })
        .await;
        assert!(matches!(error, Err(ModelError::Cancelled)), "{error:?}");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
