use std::path::PathBuf;
use std::sync::Arc;

use raygun_model::{ChipId, ChipPrefs, ChipProgress};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Serves `body` for every request, as GitHub raw would serve a package.
async fn serve(body: Vec<u8>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let body = body.clone();
            tokio::spawn(async move {
                let mut request = [0u8; 2048];
                let _ = socket.read(&mut request).await;
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
        }
    });
    format!("http://{address}/package.tar.gz")
}

fn service(dir: &std::path::Path, entries: Vec<ChipEntry>) -> Arc<ChipsService> {
    ChipsService::new(
        dir.to_owned(),
        entries,
        raygun_net::http_client().unwrap(),
        ChipPrefs::default(),
        |_| {},
    )
}

/// The catalogue entry of `id` with its package served from `url`.
fn served(id: &str, url: &str) -> ChipEntry {
    let mut entry = catalog().into_iter().find(|entry| entry.id == id).unwrap();
    entry.package.url = url.to_owned();
    entry
}

#[test]
fn every_chip_has_a_package_in_the_repository_as_the_catalogue_pins_it() {
    let entries = catalog();
    for chip in ChipId::ALL {
        let entry = entries
            .iter()
            .find(|entry| entry.id == chip.as_str())
            .unwrap_or_else(|| panic!("{} isn't in the catalogue", chip.as_str()));
        let name = entry.package.url.rsplit('/').next().unwrap();
        assert!(
            entry.package.url.starts_with(
                "https://raw.githubusercontent.com/uvxszi/raygun/main/chips/packages/"
            )
        );
        let bytes = std::fs::read(repo().join("chips/packages").join(name))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(bytes.len() as u64, entry.package.size, "{name}'s size");
        assert_eq!(
            hex(&Sha256::digest(&bytes)),
            entry.package.sha256,
            "{name} changed: run `python tools/build_chips.py`"
        );
    }
}

#[tokio::test]
async fn smooth_motion_installs_from_its_package_with_its_shaders() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = std::fs::read(repo().join("chips/packages/smooth-motion-1.0.0.tar.gz")).unwrap();
    let url = serve(bytes).await;
    let chips = service(temp.path(), vec![served("smooth-motion", &url)]);
    assert!(!chips.is_enabled(ChipId::SmoothMotion));

    chips.install(ChipId::SmoothMotion).await.unwrap();

    assert!(chips.is_installed(ChipId::SmoothMotion));
    assert!(
        chips.is_enabled(ChipId::SmoothMotion),
        "on unless switched off"
    );
    let shader = chips
        .read_text(ChipId::SmoothMotion, "fx_motion_warp.glsl")
        .await
        .unwrap();
    assert!(shader.contains("uFlowTexel"));
    assert!(
        !shader.contains('\r'),
        "text is packed with LF line endings"
    );
    let listed = chips.list();
    let info = &listed[0];
    assert!(info.installed && info.on && !info.outdated);
    assert_eq!(info.progress, ChipProgress::Idle);
    // Nothing of the download is left beside it.
    let names: Vec<String> = std::fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["smooth-motion"]);

    // Reading outside its folder, or while off, is refused.
    assert!(chips.read_text(ChipId::SmoothMotion, "../x").await.is_err());
    chips.set_prefs(ChipPrefs::default().with(ChipId::SmoothMotion, false));
    assert!(!chips.is_enabled(ChipId::SmoothMotion));
    assert!(
        chips
            .read_text(ChipId::SmoothMotion, "fx_motion_warp.glsl")
            .await
            .is_err()
    );

    // A new start finds it installed; removing takes its folder.
    let again = service(temp.path(), vec![served("smooth-motion", &url)]);
    assert!(again.is_installed(ChipId::SmoothMotion));
    again.remove(ChipId::SmoothMotion).await.unwrap();
    assert!(!again.is_installed(ChipId::SmoothMotion));
    assert!(!temp.path().join("smooth-motion").exists());
}

#[tokio::test]
async fn a_damaged_package_installs_nothing_and_says_so() {
    let temp = tempfile::tempdir().unwrap();
    let mut bytes =
        std::fs::read(repo().join("chips/packages/smart-shuffle-1.0.0.tar.gz")).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    let url = serve(bytes).await;
    let chips = service(temp.path(), vec![served("smart-shuffle", &url)]);

    let result = chips.install(ChipId::SmartShuffle).await;

    assert!(matches!(result, Err(PackageError::Mismatch)), "{result:?}");
    assert!(!chips.is_installed(ChipId::SmartShuffle));
    assert!(matches!(
        chips.list()[0].progress,
        ChipProgress::Failed { .. }
    ));
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn a_package_from_an_older_catalogue_reads_as_outdated() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("smart-subtitles");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(INSTALLED), "0000").unwrap();
    let chips = service(temp.path(), catalog());
    let info = chips
        .list()
        .into_iter()
        .find(|info| info.id == ChipId::SmartSubtitles)
        .unwrap();
    assert!(info.installed && info.outdated);
}

#[test]
fn only_paths_inside_a_chip_are_read() {
    let folder = std::path::Path::new("chips/x");
    assert!(file_in(folder, "a.glsl").is_some());
    assert!(file_in(folder, "sub/a.glsl").is_some());
    assert!(file_in(folder, "../a").is_none());
    assert!(file_in(folder, "").is_none());
    assert!(file_in(folder, "/etc/passwd").is_none());
}

#[test]
fn switches_keep_each_chip_once() {
    let prefs = ChipPrefs::default()
        .with(ChipId::SmartShuffle, false)
        .with(ChipId::SmartShuffle, false)
        .with(ChipId::SmoothMotion, false);
    assert_eq!(prefs.off, [ChipId::SmartShuffle, ChipId::SmoothMotion]);
    assert!(prefs.is_on(ChipId::SmartSubtitles));
    let prefs = prefs.with(ChipId::SmartShuffle, true);
    assert!(prefs.is_on(ChipId::SmartShuffle));
}
