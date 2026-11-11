//! A song streamed in HLS pieces (SoundCloud's) saved as one file. The pieces come through the
//! stream proxy's `/media/` route as the player gets them (the source's headers added, AES-128
//! decrypted, MPEG-TS demuxed to its sound: [`capture::bytes::hls`](super::capture::bytes::hls)),
//! are joined, and AAC goes into an M4A through the export remux, with its index first; MP3
//! pieces join into an MP3 as they are.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use tokio::sync::Notify;

use super::capture::bytes::{self, BytesError, Progress};
use crate::export::remux::{self, Source, Target};

/// Where the pieces wait, inside the song's folder.
const WORK: &str = "pieces";

#[derive(Debug, PartialEq, Eq)]
pub enum HlsError {
    Stopped,
    /// It couldn't be written here.
    Device(String),
    /// The pieces wouldn't all come, or don't make a song (a live stream, DRM).
    Failed(String),
}

/// Saves the HLS stream at `link` (a `/media/` link) as `track.<m4a|mp3|aac>` in `dir`, and gives
/// the file's name.
pub async fn save(
    client: &reqwest::Client,
    link: &str,
    dir: &Path,
    progress: Progress<'_>,
    stop: &Notify,
) -> Result<String, HlsError> {
    let work = dir.join(WORK);
    // Pieces of an earlier try don't carry over: they may be another stream's.
    let _ = tokio::fs::remove_dir_all(&work).await;
    tokio::fs::create_dir_all(&work)
        .await
        .map_err(|error| HlsError::Device(error.to_string()))?;
    let saved = match bytes::hls(client, link, &work, progress, stop).await {
        Ok(fetched) => finish(&fetched.path, dir).await,
        Err(BytesError::Stopped) => Err(HlsError::Stopped),
        Err(BytesError::Device(error)) => Err(HlsError::Device(error)),
        Err(BytesError::Failed(error)) => Err(HlsError::Failed(error)),
    };
    let _ = tokio::fs::remove_dir_all(&work).await;
    saved
}

/// The joined pieces as the song's file: AAC in an M4A, anything else as it came.
async fn finish(joined: &Path, dir: &Path) -> Result<String, HlsError> {
    let extension = joined
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("aac")
        .to_owned();
    if extension != "mp3" {
        let (source, target) = (joined.to_owned(), dir.join("track.m4a"));
        let converted = tokio::task::spawn_blocking(move || to_m4a(&source, &target))
            .await
            .map_err(|error| HlsError::Failed(error.to_string()))?;
        match converted {
            Ok(()) => return Ok("track.m4a".to_owned()),
            // Sound an M4A doesn't hold stays in the pieces' own form.
            Err(remux::Error::Unsupported(why)) => {
                tracing::info!(%why, "HLS pieces kept as they came");
            }
            Err(remux::Error::Io(error)) => return Err(HlsError::Device(error.to_string())),
            Err(other) => return Err(HlsError::Failed(other.to_string())),
        }
    }
    let file = format!("track.{extension}");
    tokio::fs::rename(joined, dir.join(&file))
        .await
        .map_err(|error| HlsError::Device(error.to_string()))?;
    Ok(file)
}

/// `source`'s sound as an M4A at `target`, written beside it first.
fn to_m4a(source: &Path, target: &Path) -> Result<(), remux::Error> {
    use std::io::Write as _;

    let partial = target.with_extension("m4a.part");
    let written = (|| {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&partial)?);
        remux::convert_with(
            &Source::File(source.to_owned()),
            Target::M4a,
            &mut out,
            &AtomicBool::new(false),
            &[],
        )?;
        out.flush()?;
        drop(out);
        std::fs::rename(&partial, target)?;
        Ok(())
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    written
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::path::PathBuf;

    use super::*;

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/export/remux/fixtures")
    }

    /// Serves the remux fixtures' folders over HTTP, as an HLS host would.
    async fn host() -> SocketAddr {
        use axum::extract::Path as UrlPath;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/{dir}/{file}",
            axum::routing::get(
                |UrlPath((dir, file)): UrlPath<(String, String)>| async move {
                    match std::fs::read(fixtures().join(dir).join(file)) {
                        Ok(bytes) => (axum::http::StatusCode::OK, bytes),
                        Err(_) => (axum::http::StatusCode::NOT_FOUND, Vec::new()),
                    }
                },
            ),
        );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        address
    }

    async fn saved(playlist: &str) -> (tempfile::TempDir, String, Option<u64>) {
        let address = host().await;
        let dir = tempfile::tempdir().unwrap();
        let client = raygun_net::http_client().unwrap();
        let progress = |_: u64, _: Option<f64>| {};
        let file = save(
            &client,
            &format!("http://{address}/{playlist}"),
            dir.path(),
            &progress,
            &Notify::new(),
        )
        .await
        .unwrap();
        let path = dir.path().join(&file);
        let measured = crate::music_downloads::transcode::measure(&path).unwrap();
        assert!(!dir.path().join(WORK).exists(), "the pieces are gone");
        (dir, file, measured)
    }

    #[tokio::test]
    async fn aac_pieces_become_an_m4a_with_its_index_first() {
        // MPEG-TS pieces (their sound alone), packed ADTS pieces, and fragmented MP4 pieces.
        for (playlist, seconds) in [
            ("ts/index.m3u8", 2.0),
            ("pair/audio.m3u8", 2.0),
            ("fmp4/index.m3u8", 2.0),
        ] {
            let (dir, file, measured) = saved(playlist).await;
            assert_eq!(file, "track.m4a", "{playlist}");
            let bytes = std::fs::read(dir.path().join(&file)).unwrap();
            let moov = bytes.windows(4).position(|window| window == b"moov");
            let mdat = bytes.windows(4).position(|window| window == b"mdat");
            assert!(moov < mdat, "{playlist}: the index comes first");
            let ms = measured.unwrap_or_default() as f64 / 1000.0;
            assert!((ms - seconds).abs() < 0.3, "{playlist}: {ms} s");
        }
    }

    /// Nuclear's SoundCloud plugin, installed from Nuclear's registry as the app installs it: a
    /// song found and its HLS stream resolved, played as the player gets it (the playlist through
    /// the proxy's `/media/` route, rewritten, and its first piece fetched through it), and saved
    /// as one file that plays the whole song:
    /// `cargo test -p raygun-app --lib -- --ignored soundcloud_hls_live --nocapture`
    /// (`NUCLEAR_PLUGINS_DIR` keeps the package between runs).
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "needs network access to jsDelivr, GitHub and SoundCloud"]
    async fn soundcloud_hls_live() {
        use raygun_ext_host::{
            ExtensionHost, ExtensionSource, HostConfig, MANIFEST_FILE, StreamUse,
        };

        const REGISTRY: &str =
            "https://cdn.jsdelivr.net/gh/NuclearPlayer/plugin-registry@master/plugins.json";
        const PLUGIN: &str = "nuclear.nuclear-plugin-soundcloud";
        let client = raygun_net::http_client().unwrap();
        let registry_url = reqwest::Url::parse(REGISTRY).unwrap();
        let index_text = client
            .get(REGISTRY)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let index = raygun_ext_host::repo::parse_index(&index_text).unwrap();
        let entry = index
            .extensions
            .iter()
            .find(|entry| entry.id == PLUGIN)
            .expect("the registry lists SoundCloud");
        let kept = std::env::var_os("NUCLEAR_PLUGINS_DIR")
            .map(|dir| PathBuf::from(dir).join(format!("{}-{}.zip", entry.id, entry.version)));
        let package = match kept.as_ref().and_then(|file| std::fs::read(file).ok()) {
            Some(bytes) => bytes,
            None => {
                let url = raygun_ext_host::repo::package_url(&registry_url, entry).unwrap();
                let bytes = client
                    .get(url)
                    .send()
                    .await
                    .unwrap()
                    .bytes()
                    .await
                    .unwrap()
                    .to_vec();
                raygun_ext_host::package::check_nuclear_download(&bytes, entry).unwrap();
                bytes
            }
        };
        let converted = raygun_ext_host::nuclear::convert(
            &raygun_ext_host::nuclear::unzip_sources(&package).unwrap(),
        )
        .unwrap();
        let host = ExtensionHost::start(HostConfig::default()).unwrap();
        for info in host
            .load(vec![ExtensionSource::built_in(&[
                (MANIFEST_FILE, &converted.manifest),
                ("dist/index.js", &converted.bundle),
            ])])
            .await
        {
            assert_eq!(
                info.status,
                raygun_model::ExtensionStatus::Loaded,
                "{:?}",
                info.error
            );
        }

        // Found and resolved as the resolver does: the first song SoundCloud streams whole (some
        // are previews of paid ones, with no HLS stream).
        let mut found = None;
        for (title, artist) in [
            ("Strobe", "deadmau5"),
            ("Nightcall", "Kavinsky"),
            ("Ghosts n Stuff", "deadmau5"),
        ] {
            let query = raygun_model::TrackQuery {
                title: title.into(),
                artist: Some(artist.into()),
                album: None,
                duration_ms: None,
            };
            let answers = host
                .stream_candidates(&query, 5, raygun_resolver::QualityTarget::Any)
                .await;
            let candidates = answers
                .into_iter()
                .find(|answer| answer.provider.extension_id == PLUGIN)
                .expect("SoundCloud answered")
                .result
                .unwrap();
            for candidate in candidates {
                let using = StreamUse {
                    quality: raygun_resolver::QualityTarget::Any,
                    decodable: true,
                };
                match host
                    .resolve_stream(&candidate.provider_id, &candidate.id, using)
                    .await
                {
                    Ok(stream) => {
                        found = Some((candidate, stream));
                        break;
                    }
                    Err(error) => println!("{}: {error}", candidate.title),
                }
            }
            if found.is_some() {
                break;
            }
        }
        let (candidate, stream) = found.expect("a song SoundCloud streams");
        let raygun_model::MediaSource::Url { url, headers } = &stream.source else {
            panic!("{:?}", stream.source);
        };
        println!(
            "{} ({:?} ms): {url}",
            candidate.title, candidate.duration_ms
        );
        assert!(raygun_net::manifest::names_hls(url), "{url}");

        // Played as the player gets it: the playlist through the proxy, its pieces too.
        let proxy = raygun_net::StreamProxy::start(client.clone(), None)
            .await
            .unwrap();
        let link = proxy.media().link(url, headers).unwrap();
        let playlist = client
            .get(&link)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(playlist.starts_with("#EXTM3U"), "{playlist}");
        let local = format!("http://{}/media/", proxy.address());
        let first = playlist
            .lines()
            .find(|line| !line.starts_with('#') && !line.trim().is_empty())
            .expect("a piece or a playlist");
        assert!(first.starts_with(&local), "rewritten: {first}");
        let piece = client.get(first).send().await.unwrap();
        assert!(piece.status().is_success(), "{}", piece.status());
        assert!(!piece.bytes().await.unwrap().is_empty());

        // Saved as one file that plays it all.
        let dir = tempfile::tempdir().unwrap();
        let progress = |_: u64, _: Option<f64>| {};
        let started = std::time::Instant::now();
        let file = save(&client, &link, dir.path(), &progress, &Notify::new())
            .await
            .unwrap();
        let path = dir.path().join(&file);
        let ms = crate::music_downloads::transcode::measure(&path)
            .unwrap()
            .unwrap_or_default();
        println!(
            "saved {file}: {} KB, {:.1} s of sound, in {:.1?}",
            std::fs::metadata(&path).unwrap().len() / 1024,
            ms as f64 / 1000.0,
            started.elapsed()
        );
        let expected = candidate.duration_ms.unwrap_or(60_000);
        assert!(
            crate::music_downloads::capture::covers_enough(ms, u64::from(expected)),
            "{ms} ms of {expected}"
        );
    }

    #[tokio::test]
    async fn a_live_stream_has_no_end_to_save() {
        // The ADTS fixture's playlist without its end.
        let live = std::fs::read_to_string(fixtures().join("pair/audio.m3u8"))
            .unwrap()
            .replace("#EXT-X-ENDLIST", "");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/live.m3u8",
            axum::routing::get(move || {
                let live = live.clone();
                async move { live }
            }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let dir = tempfile::tempdir().unwrap();
        let progress = |_: u64, _: Option<f64>| {};
        let error = save(
            &raygun_net::http_client().unwrap(),
            &format!("http://{address}/live.m3u8"),
            dir.path(),
            &progress,
            &Notify::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(
            error,
            HlsError::Failed("It's a live stream, which has no end to save".into())
        );
        assert!(!dir.path().join(WORK).exists());
    }
}
