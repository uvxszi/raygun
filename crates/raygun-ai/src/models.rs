//! The model manager: a built-in catalogue of downloadable models, each installed as a folder
//! `models/<id>/` holding exactly its files. Downloads are size-capped and SHA-256-checked, and
//! land in `<id>.part/` until every file checks out; a cancelled one stops at once and leaves
//! nothing behind. A file may be a `.tar.bz2` archive: once checked, the files and folders the
//! catalogue names are taken out of it and the archive itself isn't kept.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

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
    /// on the graphics card), `ncnn` (no runner in this build yet), `piper` and `kokoro` (voices
    /// sherpa-onnx speaks with), `sherpa-onnx` (the runtime itself).
    #[serde(default)]
    pub engine: Option<String>,
    pub description: String,
    pub license: String,
    pub license_url: String,
    pub source_url: String,
    /// What a translation model takes and writes (M46.48).
    #[serde(default)]
    pub translate: Option<TranslatePair>,
    /// The platform a runtime is built for (`android-aarch64`, `windows-x86_64`…, as
    /// [`this_target`] names them); other models run anywhere.
    #[serde(default)]
    pub target: Option<String>,
    /// How a voice model is set up, and the voices it speaks with.
    #[serde(default)]
    pub tts: Option<TtsSetup>,
    pub files: Vec<ModelFile>,
}

/// A voice model's files (names in its folder) and voices.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsSetup {
    /// The ONNX model.
    pub model: String,
    pub tokens: String,
    /// Kokoro's voice table (`voices.bin`).
    #[serde(default)]
    pub voices_file: Option<String>,
    /// Kokoro's pronunciation lists, read before espeak-ng.
    #[serde(default)]
    pub lexicon: Vec<String>,
    pub voices: Vec<VoiceInfo>,
}

/// One voice of a voice model.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInfo {
    /// Unique within its model.
    pub id: String,
    pub name: String,
    /// The language it speaks, as a BCP 47 tag (`en-US`, `de`, `pt-BR`).
    pub language: String,
    /// `female` or `male`.
    #[serde(default)]
    pub gender: Option<String>,
    /// Its speaker number in a model with several.
    #[serde(default)]
    pub speaker: u32,
    /// Kokoro: the espeak-ng language its words are read with, for languages its pronunciation
    /// lists don't cover.
    #[serde(default)]
    pub lang: Option<String>,
}

/// The platform this build runs on, as runtimes' [`ModelInfo::target`] names it.
pub fn this_target() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
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
    /// For a `.tar.bz2` archive: what's taken out of it. The archive itself isn't kept.
    #[serde(default)]
    pub unpack: Option<Unpack>,
}

/// What an archive gives the model's folder.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Unpack {
    /// A file's path in the archive → its name in the folder. In the path, `*` stands for any
    /// part of one name (`*/tokens.txt`, `*/*.onnx`); the first match wins.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// A folder in the archive → the folder it becomes, everything in it taken out.
    #[serde(default)]
    pub folders: BTreeMap<String, String>,
}

/// Written into an installed model's folder when it holds unpacked archives: their checksums,
/// one a line, so a catalogue naming another archive sees the model as not installed.
const UNPACKED: &str = ".unpacked";

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
    /// An archive that checked out but doesn't hold what the catalogue says.
    #[error("{0} couldn't be unpacked: {1}")]
    Unpack(String, String),
    /// Offline Mode turned on: the download stopped, nothing of it kept.
    #[error("Offline mode is on")]
    Offline,
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

/// Whether every file of the model is present at its exact size (an archive: everything taken
/// out of it, from the archive the catalogue names now).
pub fn is_installed(models_dir: &Path, model: &ModelInfo) -> bool {
    let dir = model_dir(models_dir, model);
    let unpacked = std::fs::read_to_string(dir.join(UNPACKED)).unwrap_or_default();
    model.files.iter().all(|file| match &file.unpack {
        None => std::fs::metadata(dir.join(&file.name)).is_ok_and(|meta| meta.len() == file.size),
        Some(unpack) => {
            unpacked.lines().any(|line| line == file.sha256)
                && unpack.files.values().all(|name| dir.join(name).is_file())
                && unpack.folders.values().all(|name| dir.join(name).is_dir())
        }
    })
}

/// The catalogue's runtime of `engine` for this platform, when there is one.
pub fn runtime_for(engine: &str) -> Option<ModelInfo> {
    let target = this_target();
    catalog().into_iter().find(|model| {
        model.kind == "runtime"
            && model.engine.as_deref() == Some(engine)
            && model.target.as_deref() == Some(target.as_str())
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
    if raygun_net::network_mode::offline() {
        return Err(ModelError::Offline);
    }
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
        if let Some(unpack) = &file.unpack {
            let archive = dir.join(&file.name);
            let (into, wanted, stop) = (dir.to_owned(), unpack.clone(), cancel.clone());
            let name = file.name.clone();
            tokio::task::spawn_blocking(move || unpack_archive(&archive, &into, &wanted, &stop))
                .await
                .map_err(|error| ModelError::Unpack(name.clone(), error.to_string()))??;
            tokio::fs::remove_file(dir.join(&file.name)).await?;
            let mut marks = tokio::fs::read_to_string(dir.join(UNPACKED))
                .await
                .unwrap_or_default();
            marks.push_str(&file.sha256);
            marks.push('\n');
            tokio::fs::write(dir.join(UNPACKED), marks).await?;
        }
    }
    Ok(())
}

/// Takes what `unpack` names out of a `.tar.bz2` archive into `dir`. Paths in the archive are
/// read as relative paths only; anything else (`..`, absolute paths, links) is passed over.
fn unpack_archive(
    archive: &Path,
    dir: &Path,
    unpack: &Unpack,
    cancel: &CancellationToken,
) -> Result<(), ModelError> {
    let name = || {
        archive
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let failed = |error: std::io::Error| ModelError::Unpack(name(), error.to_string());
    let reader = std::io::BufReader::new(std::fs::File::open(archive)?);
    let mut entries = tar::Archive::new(bzip2::read::BzDecoder::new(reader));
    let mut found: Vec<&str> = Vec::new();
    for entry in entries.entries().map_err(failed)? {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        let mut entry = entry.map_err(failed)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let Some(path) = archive_path(&entry.path().map_err(failed)?) else {
            continue;
        };
        let target = unpack
            .files
            .iter()
            .find(|(pattern, wanted)| {
                !found.contains(&wanted.as_str()) && path_matches(pattern, &path)
            })
            .map(|(_, wanted)| {
                found.push(wanted.as_str());
                PathBuf::from(wanted)
            })
            .or_else(|| {
                unpack.folders.iter().find_map(|(from, to)| {
                    let inside = path.strip_prefix(&format!("{}/", from.trim_end_matches('/')))?;
                    Some(Path::new(to).join(inside))
                })
            });
        let Some(target) = target.filter(|target| is_plain_relative(target)) else {
            continue;
        };
        let out = dir.join(target);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut file).map_err(failed)?;
    }
    if let Some(missing) = unpack
        .files
        .values()
        .find(|wanted| !found.contains(&wanted.as_str()))
    {
        return Err(ModelError::Unpack(name(), format!("{missing} isn't in it")));
    }
    if let Some(missing) = unpack.folders.values().find(|to| !dir.join(to).is_dir()) {
        return Err(ModelError::Unpack(name(), format!("{missing} isn't in it")));
    }
    Ok(())
}

/// An archive member's path as `a/b/c`, without a leading `./`; nothing for any other shape.
fn archive_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_str()?.to_owned()),
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Whether `path` is relative and only goes down (no `..`, no root or drive).
fn is_plain_relative(path: &Path) -> bool {
    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
}

/// Whether an archive path matches a pattern of the same number of names, where `*` in a name
/// stands for any part of it.
fn path_matches(pattern: &str, path: &str) -> bool {
    let (patterns, names): (Vec<&str>, Vec<&str>) =
        (pattern.split('/').collect(), path.split('/').collect());
    patterns.len() == names.len()
        && patterns
            .iter()
            .zip(&names)
            .all(|(pattern, name)| name_matches(pattern, name))
}

fn name_matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((start, end)) => {
            name.len() >= start.len() + end.len() && name.starts_with(start) && name.ends_with(end)
        }
    }
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
            () = raygun_net::network_mode::went_offline() => return Err(ModelError::Offline),
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

    #[test]
    fn voices_and_their_runtime_are_in_the_catalogue() {
        let models = catalog();
        // The runtime for every platform that runs voices, each with espeak-ng's data.
        for target in [
            "android-aarch64",
            "android-x86_64",
            "android-arm",
            "android-x86",
            "windows-x86_64",
            "windows-aarch64",
            "windows-x86",
            "linux-x86_64",
            "linux-aarch64",
            "macos-aarch64",
            "macos-x86_64",
        ] {
            let runtime = models
                .iter()
                .find(|model| model.kind == "runtime" && model.target.as_deref() == Some(target))
                .unwrap_or_else(|| panic!("no runtime for {target}"));
            assert_eq!(runtime.engine.as_deref(), Some("sherpa-onnx"));
            let unpacked: Vec<&Unpack> = runtime
                .files
                .iter()
                .map(|file| file.unpack.as_ref().expect("runtimes come as archives"))
                .collect();
            let names: Vec<&String> = unpacked
                .iter()
                .flat_map(|unpack| unpack.files.values())
                .collect();
            assert!(
                names.iter().any(|name| name.contains("sherpa-onnx-c-api")),
                "{target}"
            );
            assert!(
                names.iter().any(|name| name.contains("onnxruntime")),
                "{target}"
            );
            assert!(
                unpacked
                    .iter()
                    .any(|unpack| unpack.folders.values().any(|to| to == "espeak-ng-data")),
                "{target}"
            );
        }
        // Voice models: Piper's and Kokoro, each with voices in known languages, files that
        // give what their setup names, and open licences.
        let voices: Vec<&ModelInfo> = models.iter().filter(|model| model.kind == "tts").collect();
        assert!(
            voices
                .iter()
                .filter(|model| model.engine.as_deref() == Some("piper"))
                .count()
                >= 30
        );
        assert!(
            voices
                .iter()
                .any(|model| model.engine.as_deref() == Some("kokoro"))
        );
        let mut languages = std::collections::BTreeSet::new();
        for model in &voices {
            let setup = model.tts.as_ref().expect("a voice model's setup");
            assert!(!setup.voices.is_empty(), "{}", model.id);
            let mut ids: Vec<&str> = setup.voices.iter().map(|voice| voice.id.as_str()).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(
                ids.len(),
                setup.voices.len(),
                "{}: unique voice ids",
                model.id
            );
            for voice in &setup.voices {
                let primary = voice.language.split('-').next().unwrap();
                assert!(
                    (2..=3).contains(&primary.len())
                        && primary.chars().all(|c| c.is_ascii_lowercase()),
                    "{}: {}",
                    model.id,
                    voice.language
                );
                languages.insert(primary.to_owned());
            }
            let given: Vec<&str> = model
                .files
                .iter()
                .flat_map(|file| match &file.unpack {
                    Some(unpack) => unpack.files.values().map(String::as_str).collect(),
                    None => vec![file.name.as_str()],
                })
                .collect();
            let needed = [
                Some(&setup.model),
                Some(&setup.tokens),
                setup.voices_file.as_ref(),
            ];
            for name in needed.into_iter().flatten().chain(&setup.lexicon) {
                assert!(given.contains(&name.as_str()), "{} lacks {name}", model.id);
            }
            assert!(
                !model.license.to_ascii_uppercase().contains("-NC"),
                "{}",
                model.id
            );
        }
        assert!(languages.len() >= 25, "{languages:?}");
        assert!(runtime_for("nope").is_none());
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
            target: None,
            tts: None,
            files: vec![
                ModelFile {
                    name: "a.param".into(),
                    url: format!("{base}/a.param"),
                    sha256: sha(b"param-data"),
                    size: 10,
                    unpack: None,
                },
                ModelFile {
                    name: "a.bin".into(),
                    url: format!("{base}/a.bin"),
                    sha256: bin_hash,
                    size: bin_size,
                    unpack: None,
                },
            ],
        }
    }

    /// A `.tar.bz2` holding a voice the way sherpa-onnx packs one, and a few odd entries.
    fn voice_archive() -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        let mut add = |path: &str, data: &[u8]| {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, path, data).unwrap();
        };
        add("./voice/en_US-test-medium.onnx", b"onnx");
        add("./voice/en_US-test-medium.onnx.json", b"{}");
        add("./voice/tokens.txt", b"a 1");
        add("./voice/espeak-ng-data/phontab", b"ph");
        add("./voice/espeak-ng-data/voices/!v/Mr serious", b"v");
        add("./voice/MODEL_CARD", b"card");
        let tar = builder.into_inner().unwrap();
        let mut packed = Vec::new();
        let mut encoder = bzip2::write::BzEncoder::new(&mut packed, bzip2::Compression::fast());
        std::io::Write::write_all(&mut encoder, &tar).unwrap();
        encoder.finish().unwrap();
        packed
    }

    fn archive_model(base: &str, archive: &[u8], unpack: Unpack) -> ModelInfo {
        let mut model = model(base, String::new(), 0);
        model.files = vec![ModelFile {
            name: "voice.tar.bz2".into(),
            url: format!("{base}/voice.tar.bz2"),
            sha256: sha(archive),
            size: archive.len() as u64,
            unpack: Some(unpack),
        }];
        model
    }

    #[tokio::test]
    async fn archives_are_unpacked_and_not_kept() {
        let archive = voice_archive();
        let served = archive.clone();
        let app = Router::new().route(
            "/voice.tar.bz2",
            get(move || {
                let served = served.clone();
                async move { served }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let dir = tempfile::tempdir().unwrap();
        let client = raygun_net::http_client().unwrap();
        let unpack = Unpack {
            files: BTreeMap::from([
                ("*/*.onnx".to_owned(), "model.onnx".to_owned()),
                ("*/tokens.txt".to_owned(), "tokens.txt".to_owned()),
            ]),
            folders: BTreeMap::from([(
                "voice/espeak-ng-data".to_owned(),
                "espeak-ng-data".to_owned(),
            )]),
        };
        let voice = archive_model(&base, &archive, unpack.clone());
        assert!(!is_installed(dir.path(), &voice));
        download(
            &client,
            dir.path(),
            &voice,
            &CancellationToken::new(),
            |_, _| {},
        )
        .await
        .unwrap();
        let folder = dir.path().join("test-model");
        assert_eq!(std::fs::read(folder.join("model.onnx")).unwrap(), b"onnx");
        assert_eq!(std::fs::read(folder.join("tokens.txt")).unwrap(), b"a 1");
        assert!(folder.join("espeak-ng-data/phontab").is_file());
        assert!(folder.join("espeak-ng-data/voices/!v/Mr serious").is_file());
        assert!(
            !folder.join("voice.tar.bz2").exists(),
            "the archive isn't kept"
        );
        assert!(
            !folder.join("MODEL_CARD").exists(),
            "only what's named comes out"
        );
        assert!(is_installed(dir.path(), &voice));

        // A catalogue naming another archive sees it as not installed.
        let mut newer = voice.clone();
        newer.files[0].sha256 = sha(b"another archive");
        assert!(!is_installed(dir.path(), &newer));

        // An archive without a named file fails and leaves nothing behind.
        let mut wanting = unpack;
        wanting
            .files
            .insert("*/voices.bin".to_owned(), "voices.bin".to_owned());
        let other = tempfile::tempdir().unwrap();
        let error = download(
            &client,
            other.path(),
            &archive_model(&base, &archive, wanting),
            &CancellationToken::new(),
            |_, _| {},
        )
        .await;
        assert!(matches!(error, Err(ModelError::Unpack(..))), "{error:?}");
        assert_eq!(std::fs::read_dir(other.path()).unwrap().count(), 0);
    }

    #[test]
    fn archive_paths_are_matched_by_name_and_kept_inside() {
        assert!(path_matches("*/*.onnx", "voice/en_US-a.onnx"));
        assert!(!path_matches("*/*.onnx", "voice/en_US-a.onnx.json"));
        assert!(!path_matches("*/*.onnx", "a.onnx"));
        assert!(path_matches(
            "jniLibs/x86_64/libonnxruntime.so",
            "jniLibs/x86_64/libonnxruntime.so"
        ));
        assert!(!path_matches(
            "jniLibs/x86_64/libonnxruntime.so",
            "jniLibs/x86/libonnxruntime.so"
        ));
        assert_eq!(
            archive_path(Path::new("./jniLibs/x86/a.so")).as_deref(),
            Some("jniLibs/x86/a.so")
        );
        assert_eq!(archive_path(Path::new("../evil")), None);
        assert!(!is_plain_relative(Path::new("../evil")));
        assert!(is_plain_relative(Path::new("espeak-ng-data/lang/en")));
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
