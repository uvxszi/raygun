//! The model manager: a built-in catalogue of downloadable models, each installed as a folder
//! `models/<id>/` holding exactly its files. Downloads are size-capped and SHA-256-checked, and
//! land in `<id>.part/` until every file checks out.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub description: String,
    pub license: String,
    pub license_url: String,
    pub source_url: String,
    pub files: Vec<ModelFile>,
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
/// installs it. On any failure nothing is left behind.
pub async fn download(
    client: &reqwest::Client,
    models_dir: &Path,
    model: &ModelInfo,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), ModelError> {
    let target = model_dir(models_dir, model);
    let partial = models_dir.join(format!("{}.part", model.id));
    let _ = tokio::fs::remove_dir_all(&partial).await;
    tokio::fs::create_dir_all(&partial).await?;
    if let Err(error) = fetch_all(client, &partial, model, &mut progress).await {
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
        let response = client.get(&file.url).send().await?.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > file.size)
        {
            return Err(ModelError::TooLarge(file.name.clone()));
        }
        let mut out = tokio::fs::File::create(dir.join(&file.name)).await?;
        let mut hasher = Sha256::new();
        let mut written = 0u64;
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk?;
            written += chunk.len() as u64;
            if written > file.size {
                return Err(ModelError::TooLarge(file.name.clone()));
            }
            hasher.update(&chunk);
            out.write_all(&chunk).await?;
            received += chunk.len() as u64;
            progress(received, total);
        }
        out.flush().await?;
        if written != file.size || hex(&hasher.finalize()) != file.sha256 {
            return Err(ModelError::Mismatch(file.name.clone()));
        }
    }
    Ok(())
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
    }

    fn sha(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }

    async fn serve() -> String {
        let app = Router::new()
            .route("/a.param", get(|| async { "param-data" }))
            .route("/a.bin", get(|| async { vec![7u8; 5000] }));
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
            description: String::new(),
            license: "MIT".into(),
            license_url: "https://example.com".into(),
            source_url: "https://example.com".into(),
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

        let good = model(&base, sha(&[7u8; 5000]), 5000);
        let mut seen = Vec::new();
        download(&client, dir.path(), &good, |received, total| {
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
        let error = download(&client, dir.path(), &wrong_hash, |_, _| {}).await;
        assert!(matches!(error, Err(ModelError::Mismatch(_))), "{error:?}");
        let too_long = model(&base, sha(&[7u8; 5000]), 4000);
        let error = download(&client, dir.path(), &too_long, |_, _| {}).await;
        assert!(matches!(error, Err(ModelError::TooLarge(_))), "{error:?}");
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            0,
            "nothing left behind"
        );
    }
}
