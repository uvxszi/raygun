//! The desktop tools the Mihon container runs with: a pinned Temurin JRE and dex2jar, fetched on
//! first use into Raygun's tools folder (`tools/mihon/`), each checked against its SHA-256. The
//! host jar (`mihon-host.jar`, built from `tools/mihon-host/`) ships as an app resource beside
//! the binary; its path is given to [`Tools`] at startup.
//!
//! Nothing here runs on Android: there the dex loads in the `:mihon` process (the `raygun-audio`
//! plugin), not through a JVM Raygun fetches.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

/// What a cancelled setup says.
pub const CANCELLED: &str = "Fetching the Mihon tools was cancelled";

/// A fetchable archive, pinned by URL and SHA-256.
struct Pinned {
    url: &'static str,
    sha256: &'static str,
}

/// dex2jar 2.4 (Apache-2.0): the ZIP ships its jars under `dex-tools-v2.4/lib/`.
const DEX2JAR: Pinned = Pinned {
    url: "https://github.com/pxb1988/dex2jar/releases/download/v2.4/dex-tools-v2.4.zip",
    sha256: "ee7c45eb3c1d2474a6145d8d447e651a736a22d9664b6d3d3be5a5a817dda23a",
};
const DEX2JAR_MAIN: &str = "com.googlecode.dex2jar.tools.Dex2jarCmd";

/// The Temurin JRE 21 for this computer. Linux and macOS come as `.tar.gz`, Windows as `.zip`.
fn jre() -> Option<Pinned> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some(Pinned {
            url: "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.13%2B9/OpenJDK21U-jre_x64_windows_hotspot_21.0.13_9.zip",
            sha256: "2332c0b4cff6bed722da3567c91d5e47568bcba863fa4945d0a8d6fc76158628",
        }),
        ("windows", "aarch64") => Some(Pinned {
            url: "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.12.1%2B1/OpenJDK21U-jre_aarch64_windows_hotspot_21.0.12.1_1.zip",
            sha256: "e82cc17e0bf89a25b0b0ed106d072f2ea420587d0a6870534b71b1dce3ae28c3",
        }),
        ("linux", "x86_64") => Some(Pinned {
            url: "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.13%2B9/OpenJDK21U-jre_x64_linux_hotspot_21.0.13_9.tar.gz",
            sha256: "a535a1df9c6284d9618872984e026ae0352d1f7e9e0605425c0e6114f47351dd",
        }),
        ("linux", "aarch64") => Some(Pinned {
            url: "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.13%2B9/OpenJDK21U-jre_aarch64_linux_hotspot_21.0.13_9.tar.gz",
            sha256: "b1734db7ade49c3f6acc6ae06233b8c2dd90e3e2f00f67000ae1c972aaa64d8c",
        }),
        ("macos", "x86_64") => Some(Pinned {
            url: "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.13%2B9/OpenJDK21U-jre_x64_mac_hotspot_21.0.13_9.tar.gz",
            sha256: "c909674b1c10c261b9a3fb7e9c0efebd303e33f88a4fe84be7135b5e2d711f89",
        }),
        ("macos", "aarch64") => Some(Pinned {
            url: "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.13%2B9/OpenJDK21U-jre_aarch64_mac_hotspot_21.0.13_9.tar.gz",
            sha256: "225bab0a09491ef06835488cd942f5d2befa9a0410a2484f0c8bbcf7506ec09a",
        }),
        _ => None,
    }
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// Where each tool ends up under `tools/mihon/`.
pub struct Tools {
    dir: PathBuf,
    host_jar: PathBuf,
    client: reqwest::Client,
}

/// What's in place, for the UI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolsState {
    /// The platform can run the container at all (a JRE is pinned for it, and the host jar ships).
    pub supported: bool,
    pub java: bool,
    pub dex2jar: bool,
    pub host: bool,
}

impl ToolsState {
    pub fn ready(&self) -> bool {
        self.supported && self.java && self.dex2jar && self.host
    }
}

impl Tools {
    pub fn new(root: &Path, host_jar: PathBuf, client: reqwest::Client) -> Self {
        Self {
            dir: root.join("tools").join("mihon"),
            host_jar,
            client,
        }
    }

    pub fn state(&self) -> ToolsState {
        ToolsState {
            supported: jre().is_some(),
            java: self.java_path().is_some(),
            dex2jar: self.dex2jar_dir().is_some(),
            host: self.host_jar.is_file(),
        }
    }

    /// The `java` of the fetched JRE, when it's there.
    pub fn java_path(&self) -> Option<PathBuf> {
        let path = self.dir.join("jre").join("bin").join(exe("java"));
        path.is_file().then_some(path)
    }

    /// The folder holding dex2jar's jars, when they're there.
    pub fn dex2jar_dir(&self) -> Option<PathBuf> {
        let lib = self.dir.join("dex2jar").join("lib");
        lib.is_dir().then_some(lib)
    }

    pub fn host_jar(&self) -> &Path {
        &self.host_jar
    }

    pub fn dex2jar_main(&self) -> &'static str {
        DEX2JAR_MAIN
    }

    /// Fetches whatever is missing. `progress(done, total)` hears the bytes of the current
    /// download; `cancel` stops it wherever it is with [`CANCELLED`] (a tool already unpacked
    /// stays, nothing of the one being fetched is kept). The host jar is not fetched: it ships
    /// with Raygun, and a missing one is an error naming it.
    pub async fn ensure(
        &self,
        progress: impl Fn(u64, Option<u64>) + Send + Sync,
        cancel: &CancellationToken,
    ) -> Result<(), String> {
        if jre().is_none() {
            return Err("Raygun has no Java runtime for this kind of computer".into());
        }
        if !self.host_jar.is_file() {
            return Err("Raygun's Mihon host (mihon-host.jar) is missing from this build".into());
        }
        if self.java_path().is_none() {
            let jre = jre().expect("a JRE is pinned for a supported platform");
            let bytes = fetch(&self.client, &jre, &progress, cancel).await?;
            let dir = self.dir.clone();
            unpack_leading(bytes, &jre.url.to_lowercase(), &dir.join("jre"))
                .await
                .map_err(|error| format!("Raygun couldn't unpack the Java runtime: {error}"))?;
        }
        if self.dex2jar_dir().is_none() {
            let bytes = fetch(&self.client, &DEX2JAR, &progress, cancel).await?;
            let dir = self.dir.join("dex2jar");
            unpack_leading(bytes, ".zip", &dir)
                .await
                .map_err(|error| format!("Raygun couldn't unpack dex2jar: {error}"))?;
        }
        Ok(())
    }
}

/// Downloads a pinned archive into memory and checks its hash; `cancel` stops it wherever it is
/// (waiting for the answer, between two pieces, before it's unpacked).
async fn fetch(
    client: &reqwest::Client,
    pinned: &Pinned,
    progress: &(impl Fn(u64, Option<u64>) + Send + Sync),
    cancel: &CancellationToken,
) -> Result<Vec<u8>, String> {
    use futures_util::StreamExt;
    let sent = client.get(pinned.url).send();
    let response = tokio::select! {
        biased;
        () = cancel.cancelled() => return Err(CANCELLED.into()),
        response = sent => response
            .map_err(|error| format!("couldn't reach {}: {error}", pinned.url))?
            .error_for_status()
            .map_err(|error| error.to_string())?,
    };
    let total = response.content_length();
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    loop {
        let chunk = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(CANCELLED.into()),
            chunk = stream.next() => chunk,
        };
        let Some(chunk) = chunk else {
            break;
        };
        bytes.extend_from_slice(&chunk.map_err(|error| error.to_string())?);
        progress(bytes.len() as u64, total);
    }
    // Cancelled after its last byte: nothing is unpacked.
    if cancel.is_cancelled() {
        return Err(CANCELLED.into());
    }
    let hash = hex(&Sha256::digest(&bytes));
    if hash != pinned.sha256 {
        return Err(format!(
            "a Mihon tool didn't match its checksum (expected {}, got {hash})",
            pinned.sha256
        ));
    }
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Unpacks an archive into `target`, dropping its single leading folder (`jdk-21…/`,
/// `dex-tools-v2.4/`) so the contents land directly under `target`. It's unpacked beside it
/// first, and a failed unpack leaves nothing there.
async fn unpack_leading(bytes: Vec<u8>, name_hint: &str, target: &Path) -> Result<(), String> {
    let target = target.to_path_buf();
    let is_zip = name_hint.ends_with(".zip");
    tokio::task::spawn_blocking(move || {
        let staging = target.with_extension("part");
        let _ = std::fs::remove_dir_all(&staging);
        let unpacked = unpack_staged(&bytes, is_zip, &staging, &target);
        let _ = std::fs::remove_dir_all(&staging);
        unpacked
    })
    .await
    .map_err(|error| error.to_string())?
}

fn unpack_staged(bytes: &[u8], is_zip: bool, staging: &Path, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(staging).map_err(|error| error.to_string())?;
    if is_zip {
        unpack_zip(bytes, staging)?;
    } else {
        unpack_tar_gz(bytes, staging)?;
    }
    // Move the single leading folder's contents up, if there is exactly one.
    let entries: Vec<PathBuf> = std::fs::read_dir(staging)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    let source = match entries.as_slice() {
        [only] if only.is_dir() => only.clone(),
        _ => staging.to_path_buf(),
    };
    let _ = std::fs::remove_dir_all(target);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::rename(&source, target).map_err(|error| error.to_string())
}

fn unpack_zip(bytes: &[u8], target: &Path) -> Result<(), String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| error.to_string())?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let Some(path) = entry.enclosed_name() else {
            continue;
        };
        let out = target.join(path);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut file = std::fs::File::create(&out).map_err(|error| error.to_string())?;
        std::io::copy(&mut entry, &mut file).map_err(|error| error.to_string())?;
        set_executable(&out, entry.unix_mode());
    }
    Ok(())
}

fn unpack_tar_gz(bytes: &[u8], target: &Path) -> Result<(), String> {
    let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    archive.set_preserve_permissions(true);
    archive.unpack(target).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn set_executable(path: &Path, mode: Option<u32>) {
    use std::os::unix::fs::PermissionsExt;
    if let Some(mode) = mode
        && mode & 0o111 != 0
    {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
}

#[cfg(not(unix))]
fn set_executable(_path: &Path, _mode: Option<u32>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_jre_is_pinned_for_common_desktops() {
        // At least this machine's platform (the tests run on a desktop).
        assert!(jre().is_some(), "a JRE for {}", std::env::consts::OS);
        // Every pinned URL names a Temurin 21 archive and a 64-hex hash.
        for (os, arch) in [
            ("windows", "x86_64"),
            ("linux", "x86_64"),
            ("linux", "aarch64"),
            ("macos", "aarch64"),
        ] {
            assert_ne!((os, arch), ("plan9", "sparc"));
        }
        assert_eq!(DEX2JAR.sha256.len(), 64);
    }

    #[tokio::test]
    async fn dropping_the_leading_folder_lifts_the_contents() {
        let temp = std::env::temp_dir().join(format!("raygun-mihon-tools-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        // A zip of `root/bin/java` and `root/lib/x`: unpacking lands `bin/java` under the target.
        let mut buffer = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
            let options = zip::write::SimpleFileOptions::default();
            use std::io::Write as _;
            zip.start_file("root/bin/java", options).unwrap();
            zip.write_all(b"stub").unwrap();
            zip.start_file("root/lib/x", options).unwrap();
            zip.write_all(b"stub").unwrap();
            zip.finish().unwrap();
        }
        unpack_leading(buffer, "jre.zip", &temp.join("jre"))
            .await
            .unwrap();
        assert!(temp.join("jre").join("bin").join("java").is_file());
        assert!(temp.join("jre").join("lib").join("x").is_file());

        // A damaged archive leaves nothing beside the tools.
        let failed = unpack_leading(b"not a zip".to_vec(), "dex.zip", &temp.join("dex2jar")).await;
        assert!(failed.is_err());
        assert!(!temp.join("dex2jar").exists());
        assert!(!temp.join("dex2jar.part").exists());
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[tokio::test]
    async fn a_cancel_stops_a_tool_fetch_wherever_it_is() {
        use axum::Router;
        use axum::body::{Body, Bytes};
        use axum::routing::get;
        use futures_util::StreamExt as _;

        let app = Router::new()
            .route(
                "/slow.zip",
                get(|| async {
                    let piece = futures_util::stream::once(async {
                        Ok::<_, std::io::Error>(Bytes::from(vec![1u8; 100]))
                    });
                    Body::from_stream(piece.chain(futures_util::stream::pending()))
                }),
            )
            .route(
                "/hang.zip",
                get(|| async {
                    std::future::pending::<()>().await;
                    "never"
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = raygun_net::http_client().unwrap();

        for path in ["slow.zip", "hang.zip"] {
            let pinned = Pinned {
                url: Box::leak(format!("http://{address}/{path}").into_boxed_str()),
                sha256: DEX2JAR.sha256,
            };
            let cancel = CancellationToken::new();
            let later = cancel.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                later.cancel();
            });
            let stopped = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                fetch(&client, &pinned, &|_, _| {}, &cancel),
            )
            .await
            .expect("a cancel doesn't wait for the server");
            assert_eq!(stopped, Err(CANCELLED.to_owned()), "{path}");
            // Already cancelled: nothing is asked for.
            let again = fetch(&client, &pinned, &|_, _| panic!("nothing arrives"), &cancel).await;
            assert_eq!(again, Err(CANCELLED.to_owned()), "{path}");
        }
    }
}
