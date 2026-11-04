//! yt-dlp (M46.47): YouTube videos and their sound saved on desktop, and songs only YouTube has
//! saved as files instead of recorded as they play.
//!
//! yt-dlp is looked for in `<data root>/tools/`, then on the PATH. When it's missing it is fetched
//! from its GitHub releases the first time it's needed and checked against the release's
//! `SHA2-256SUMS`. FFmpeg (which joins YouTube's separate picture and sound above 360p) is used
//! when found the same way, and on Windows it can be fetched from yt-dlp's FFmpeg builds. Phones
//! have neither: everything here says it isn't supported there.
//!
//! Videos and sound go to the user's `Downloads/Raygun/YouTube/`, one at a time, listed in
//! Downloads → Exports. yt-dlp runs without a console window and is stopped with the job.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use raygun_model::{YouTubeLink, YouTubeLinkKind, YtDlpJob, YtDlpKind, YtDlpState, YtDlpStatus};
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Notify;

use crate::events::YtDlpEvent;

const RELEASES: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
const FFMPEG_RELEASES: &str = "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest";
const FFMPEG_ZIP: &str = "ffmpeg-master-latest-win64-gpl.zip";
/// yt-dlp's own file for this system.
const ASSET: &str = if cfg!(windows) {
    "yt-dlp.exe"
} else if cfg!(target_os = "macos") {
    "yt-dlp_macos"
} else {
    "yt-dlp_linux"
};
/// The biggest yt-dlp or FFmpeg download accepted.
const MAX_TOOL_BYTES: u64 = 400 << 20;
/// A song that takes longer than this has failed.
const SONG_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Finished jobs kept in the list.
const KEPT_JOBS: usize = 40;
/// Marks Raygun's own lines among yt-dlp's output.
const PROGRESS: &str = "RAYGUN_PROGRESS";
const FILE: &str = "RAYGUN_FILE";

/// Whether yt-dlp can run here: desktop only.
pub const fn supported() -> bool {
    !cfg!(any(target_os = "android", target_os = "ios"))
}

struct Job {
    info: YtDlpJob,
    cancel: Arc<Notify>,
    cancelled: Arc<AtomicBool>,
    path: Option<PathBuf>,
}

#[derive(Default)]
struct Found {
    /// yt-dlp's path and version, once looked for.
    program: Option<(PathBuf, String)>,
    installing: bool,
    message: Option<String>,
}

pub struct YtDlpService {
    app: AppHandle,
    tools: PathBuf,
    exports: PathBuf,
    client: reqwest::Client,
    found: Mutex<Found>,
    /// One fetch of yt-dlp or FFmpeg at a time.
    install_turn: tokio::sync::Mutex<()>,
    /// One download at a time.
    turn: tokio::sync::Mutex<()>,
    jobs: Mutex<Vec<Job>>,
}

impl YtDlpService {
    pub fn new(app: AppHandle, root: &Path, client: reqwest::Client) -> Arc<Self> {
        Arc::new(Self {
            app,
            tools: root.join("tools"),
            exports: root.join("exports"),
            client,
            found: Mutex::new(Found::default()),
            install_turn: tokio::sync::Mutex::new(()),
            turn: tokio::sync::Mutex::new(()),
            jobs: Mutex::new(Vec::new()),
        })
    }

    /// What's installed, without fetching anything.
    pub async fn status(&self) -> YtDlpStatus {
        if !supported() {
            return YtDlpStatus::default();
        }
        let version = self.program().await.map(|(_, version)| version);
        let found = self.found.lock().expect("yt-dlp state poisoned");
        YtDlpStatus {
            supported: true,
            version,
            ffmpeg: self.ffmpeg().is_some(),
            installing: found.installing,
            message: found.message.clone(),
        }
    }

    /// yt-dlp's path and version: Raygun's own copy, else one on the PATH.
    async fn program(&self) -> Option<(PathBuf, String)> {
        if let Some(found) = self
            .found
            .lock()
            .expect("yt-dlp state poisoned")
            .program
            .clone()
            && found.0.is_file()
        {
            return Some(found);
        }
        let candidates = [Some(self.tools.join(exe("yt-dlp"))), on_path("yt-dlp")];
        for path in candidates.into_iter().flatten() {
            if !path.is_file() {
                continue;
            }
            let output = tokio::time::timeout(
                Duration::from_secs(20),
                command(&path).arg("--version").output(),
            )
            .await;
            if let Ok(Ok(output)) = output
                && output.status.success()
            {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                let found = (path, version);
                self.found.lock().expect("yt-dlp state poisoned").program = Some(found.clone());
                return Some(found);
            }
        }
        None
    }

    /// FFmpeg's folder, when it's in Raygun's tools or on the PATH.
    fn ffmpeg(&self) -> Option<PathBuf> {
        if self.tools.join(exe("ffmpeg")).is_file() {
            return Some(self.tools.clone());
        }
        on_path("ffmpeg").and_then(|path| path.parent().map(Path::to_path_buf))
    }

    /// yt-dlp, fetched first when it isn't here.
    async fn ready(&self) -> Result<PathBuf, String> {
        if !supported() {
            return Err("YouTube downloads need Raygun on a computer".into());
        }
        if let Some((path, _)) = self.program().await {
            return Ok(path);
        }
        self.install().await?;
        self.program()
            .await
            .map(|(path, _)| path)
            .ok_or_else(|| "yt-dlp was fetched but doesn't start".into())
    }

    /// Fetches yt-dlp's latest release into Raygun's tools, checked against its checksums.
    pub async fn install(&self) -> Result<YtDlpStatus, String> {
        if !supported() {
            return Err("yt-dlp runs on computers only".into());
        }
        let _turn = self.install_turn.lock().await;
        self.set_installing(true, None);
        let result = self.fetch_ytdlp().await;
        self.set_installing(false, result.as_ref().err().cloned());
        result?;
        Ok(self.status().await)
    }

    async fn fetch_ytdlp(&self) -> Result<(), String> {
        let sums = self.text(&format!("{RELEASES}/SHA2-256SUMS")).await?;
        let expected = checksum_for(&sums, ASSET)
            .ok_or_else(|| format!("yt-dlp's checksums don't list {ASSET}"))?;
        let bytes = self.bytes(&format!("{RELEASES}/{ASSET}")).await?;
        if sha256_hex(&bytes) != expected {
            return Err("yt-dlp's download doesn't match its checksum".into());
        }
        let target = self.tools.join(exe("yt-dlp"));
        write_tool(&target, &bytes).await?;
        self.found.lock().expect("yt-dlp state poisoned").program = None;
        tracing::info!(path = %target.display(), "yt-dlp installed");
        Ok(())
    }

    /// Fetches FFmpeg (Windows: yt-dlp's builds) for videos above 360p.
    pub async fn install_ffmpeg(&self) -> Result<YtDlpStatus, String> {
        if !cfg!(windows) {
            return Err("Install FFmpeg with your system's package manager".into());
        }
        let _turn = self.install_turn.lock().await;
        self.set_installing(true, None);
        let result = self.fetch_ffmpeg().await;
        self.set_installing(false, result.as_ref().err().cloned());
        result?;
        Ok(self.status().await)
    }

    async fn fetch_ffmpeg(&self) -> Result<(), String> {
        let sums = self
            .text(&format!("{FFMPEG_RELEASES}/checksums.sha256"))
            .await?;
        let expected = checksum_for(&sums, FFMPEG_ZIP)
            .ok_or_else(|| "FFmpeg's checksums don't list the Windows build".to_owned())?;
        let bytes = self
            .bytes(&format!("{FFMPEG_RELEASES}/{FFMPEG_ZIP}"))
            .await?;
        if sha256_hex(&bytes) != expected {
            return Err("FFmpeg's download doesn't match its checksum".into());
        }
        let tools = self.tools.clone();
        tokio::task::spawn_blocking(move || unpack_ffmpeg(&bytes, &tools))
            .await
            .map_err(|error| error.to_string())??;
        tracing::info!("FFmpeg installed");
        Ok(())
    }

    fn set_installing(&self, installing: bool, message: Option<String>) {
        {
            let mut found = self.found.lock().expect("yt-dlp state poisoned");
            found.installing = installing;
            found.message = message;
        }
        let _ = YtDlpEvent(String::new()).emit(&self.app);
    }

    async fn text(&self, url: &str) -> Result<String, String> {
        let bytes = self.bytes(url).await?;
        String::from_utf8(bytes).map_err(|error| error.to_string())
    }

    async fn bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(600))
            .send()
            .await
            .map_err(|error| format!("{url}: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("{url}: HTTP {}", response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_TOOL_BYTES)
        {
            return Err(format!("{url} is too big"));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("{url}: {error}"))?;
        if bytes.len() as u64 > MAX_TOOL_BYTES {
            return Err(format!("{url} is too big"));
        }
        Ok(bytes.to_vec())
    }

    // ---- Songs (only YouTube has them) ----

    /// Saves a YouTube video's sound as `<dir>/track.<ext>` (M4A where YouTube has it) for the
    /// song downloads; the file's name comes back. `progress` hears the bytes saved and the size
    /// when yt-dlp knows it; `cancel` stops yt-dlp (what it saved stays for the next go).
    pub async fn fetch_song(
        &self,
        video_id: &str,
        dir: &Path,
        cancel: Arc<Notify>,
        progress: impl Fn(u64, Option<u64>) + Send + Sync + 'static,
    ) -> Result<String, String> {
        let program = self.ready().await?;
        let url = watch_url(video_id);
        let mut command = command(&program);
        command
            .args([
                "--no-playlist",
                "--no-warnings",
                "--newline",
                "--progress",
                "--progress-template",
                &format!(
                    "download:{PROGRESS} %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s"
                ),
                "-f",
                "bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]/best[ext=m4a]",
                "-o",
            ])
            .arg(dir.join("track.%(ext)s"))
            .args(["--print", &format!("after_move:{FILE} %(filepath)s")])
            .arg("--")
            .arg(&url);
        let run = run(command, cancel, move |numbers| {
            if let Some((done, total)) = progress_bytes(numbers) {
                progress(done, total);
            }
        });
        let path = tokio::time::timeout(SONG_TIMEOUT, run)
            .await
            .map_err(|_| "yt-dlp took too long".to_owned())??;
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| "yt-dlp didn't say where it saved the song".into())
    }

    // ---- Downloads from a YouTube address ----

    /// Starts saving a YouTube video (or its sound) to `Downloads/Raygun/YouTube/`; the job's id
    /// comes back at once.
    pub fn download(
        self: &Arc<Self>,
        url: &str,
        title: &str,
        kind: YtDlpKind,
    ) -> Result<String, String> {
        if !supported() {
            return Err("YouTube downloads need Raygun on a computer".into());
        }
        let link = parse_link(url).ok_or("That isn't a YouTube video's address")?;
        if link.kind != YouTubeLinkKind::Video {
            return Err("Only single videos can be downloaded".into());
        }
        let id = uuid::Uuid::now_v7().to_string();
        let job = Job {
            info: YtDlpJob {
                id: id.clone(),
                url: link.url,
                title: if title.trim().is_empty() {
                    link.id
                } else {
                    title.trim().to_owned()
                },
                kind,
                state: YtDlpState::Queued,
                progress: None,
                file: None,
                message: None,
            },
            cancel: Arc::new(Notify::new()),
            cancelled: Arc::new(AtomicBool::new(false)),
            path: None,
        };
        {
            let mut jobs = self.jobs.lock().expect("yt-dlp jobs poisoned");
            jobs.insert(0, job);
            let finished: Vec<usize> = jobs
                .iter()
                .enumerate()
                .filter(|(_, job)| {
                    !matches!(job.info.state, YtDlpState::Queued | YtDlpState::Downloading)
                })
                .map(|(index, _)| index)
                .skip(KEPT_JOBS)
                .collect();
            for index in finished.into_iter().rev() {
                jobs.remove(index);
            }
        }
        self.changed(&id);
        let service = self.clone();
        let job_id = id.clone();
        tauri::async_runtime::spawn(async move { service.run_job(&job_id).await });
        Ok(id)
    }

    async fn run_job(self: Arc<Self>, id: &str) {
        let _turn = self.turn.lock().await;
        let Some((url, kind, cancel, cancelled)) = self.with_job(id, |job| {
            (
                job.info.url.clone(),
                job.info.kind,
                job.cancel.clone(),
                job.cancelled.clone(),
            )
        }) else {
            return;
        };
        if cancelled.load(Ordering::SeqCst) {
            return;
        }
        self.update(id, |job| job.info.state = YtDlpState::Downloading);
        let result = self.save(id, &url, kind, cancel).await;
        self.update(id, |job| {
            if job.cancelled.load(Ordering::SeqCst) {
                job.info.state = YtDlpState::Cancelled;
                return;
            }
            match &result {
                Ok(path) => {
                    job.info.state = YtDlpState::Done;
                    job.info.progress = Some(1.0);
                    job.info.file = Some(shown_path(path));
                    job.path = Some(path.clone());
                }
                Err(message) => {
                    job.info.state = YtDlpState::Failed;
                    job.info.message = Some(message.clone());
                }
            }
        });
    }

    async fn save(
        self: &Arc<Self>,
        id: &str,
        url: &str,
        kind: YtDlpKind,
        cancel: Arc<Notify>,
    ) -> Result<PathBuf, String> {
        let program = self.ready().await?;
        #[cfg(not(target_os = "android"))]
        let folder =
            crate::export::output::desktop_folder(&crate::export::Place::Downloads, &self.exports)
                .join("YouTube");
        #[cfg(target_os = "android")]
        let folder = self.exports.join("YouTube");
        tokio::fs::create_dir_all(&folder)
            .await
            .map_err(|error| error.to_string())?;
        let ffmpeg = self.ffmpeg();
        let format = match (kind, &ffmpeg) {
            (YtDlpKind::Audio, _) => "bestaudio[ext=m4a]/bestaudio",
            // Picture and sound joined into one MP4, up to 1080p.
            (YtDlpKind::Video, Some(_)) => {
                "bv*[height<=1080][ext=mp4]+ba[ext=m4a]/bv*[height<=1080]+ba/b"
            }
            // Without FFmpeg only files that already hold both (360p on most videos).
            (YtDlpKind::Video, None) => "b[ext=mp4]/b",
        };
        let mut command = command(&program);
        command.args([
            "--no-playlist",
            "--no-warnings",
            "--newline",
            "--progress",
            "--progress-template",
            &format!(
                "download:{PROGRESS} %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s"
            ),
            "-f",
            format,
            "--windows-filenames",
            "-o",
        ]);
        command.arg(folder.join("%(title).120B [%(id)s].%(ext)s"));
        if let Some(dir) = &ffmpeg {
            command.arg("--ffmpeg-location").arg(dir);
            if kind == YtDlpKind::Video {
                command.args(["--merge-output-format", "mp4"]);
            }
        }
        command
            .args(["--print", &format!("after_move:{FILE} %(filepath)s")])
            .arg("--")
            .arg(url);
        let service = self.clone();
        let job = id.to_owned();
        run(command, cancel, move |numbers| {
            let Some(progress) = progress_share(numbers) else {
                return;
            };
            // yt-dlp reports every block: the list hears each whole percent.
            let shown = service
                .with_job(&job, |job| job.info.progress)
                .flatten()
                .map(|shown| (shown * 100.0) as u32);
            if shown != Some((progress * 100.0) as u32) {
                service.update(&job, |job| job.info.progress = Some(progress));
            }
        })
        .await
    }

    pub fn jobs(&self) -> Vec<YtDlpJob> {
        self.jobs
            .lock()
            .expect("yt-dlp jobs poisoned")
            .iter()
            .map(|job| job.info.clone())
            .collect()
    }

    /// Stops a job (yt-dlp is ended) or takes a finished one off the list (its file stays).
    pub fn cancel(&self, id: &str) {
        let running = self.with_job(id, |job| {
            matches!(job.info.state, YtDlpState::Queued | YtDlpState::Downloading)
        });
        match running {
            Some(true) => {
                self.update(id, |job| {
                    job.cancelled.store(true, Ordering::SeqCst);
                    job.cancel.notify_one();
                    job.info.state = YtDlpState::Cancelled;
                });
            }
            Some(false) => {
                self.jobs
                    .lock()
                    .expect("yt-dlp jobs poisoned")
                    .retain(|job| job.info.id != id);
                self.changed(id);
            }
            None => {}
        }
    }

    /// Shows a finished job's file in the file manager.
    pub fn reveal(&self, id: &str) -> Result<(), String> {
        let path = self
            .with_job(id, |job| job.path.clone())
            .flatten()
            .ok_or("That download has no file yet")?;
        #[cfg(not(target_os = "android"))]
        return crate::export::output::reveal(&path);
        #[cfg(target_os = "android")]
        {
            let _ = path;
            Err("Not on phones".into())
        }
    }

    fn with_job<T>(&self, id: &str, read: impl FnOnce(&Job) -> T) -> Option<T> {
        self.jobs
            .lock()
            .expect("yt-dlp jobs poisoned")
            .iter()
            .find(|job| job.info.id == id)
            .map(read)
    }

    fn update(&self, id: &str, change: impl FnOnce(&mut Job)) {
        {
            let mut jobs = self.jobs.lock().expect("yt-dlp jobs poisoned");
            let Some(job) = jobs.iter_mut().find(|job| job.info.id == id) else {
                return;
            };
            change(job);
        }
        self.changed(id);
    }

    fn changed(&self, id: &str) {
        let _ = YtDlpEvent(id.to_owned()).emit(&self.app);
    }
}

/// Runs yt-dlp, handing each progress line's numbers to `progress` as it reads, until it ends or
/// `cancel` fires; the saved file's path comes back.
async fn run(
    mut command: tokio::process::Command,
    cancel: Arc<Notify>,
    progress: impl Fn(&str) + Send + Sync + 'static,
) -> Result<PathBuf, String> {
    let mut child = command
        .spawn()
        .map_err(|error| format!("yt-dlp didn't start: {error}"))?;
    let stdout = child.stdout.take().ok_or("yt-dlp gave no output")?;
    let stderr = child.stderr.take().ok_or("yt-dlp gave no output")?;
    let saved: Arc<Mutex<Option<PathBuf>>> = Arc::default();
    let errors: Arc<Mutex<Vec<String>>> = Arc::default();
    let progress = Arc::new(progress);
    let read = |stream: Box<dyn tokio::io::AsyncRead + Unpin + Send>| {
        let (saved, errors, progress) = (saved.clone(), errors.clone(), progress.clone());
        tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim();
                if let Some(path) = line.strip_prefix(FILE) {
                    *saved.lock().expect("yt-dlp output poisoned") =
                        Some(PathBuf::from(path.trim()));
                } else if let Some(numbers) = line.strip_prefix(PROGRESS) {
                    progress(numbers);
                } else if line.starts_with("ERROR:") {
                    errors
                        .lock()
                        .expect("yt-dlp output poisoned")
                        .push(line.trim_start_matches("ERROR:").trim().to_owned());
                }
            }
        })
    };
    let readers = [read(Box::new(stdout)), read(Box::new(stderr))];
    let status = tokio::select! {
        status = child.wait() => status.map_err(|error| error.to_string())?,
        () = cancel.notified() => {
            let _ = child.kill().await;
            return Err("Cancelled".into());
        }
    };
    for reader in readers {
        let _ = reader.await;
    }
    let path = saved.lock().expect("yt-dlp output poisoned").take();
    match path {
        Some(path) if status.success() => Ok(path),
        _ => {
            let errors = errors.lock().expect("yt-dlp output poisoned");
            Err(errors
                .last()
                .cloned()
                .unwrap_or_else(|| format!("yt-dlp stopped ({status})")))
        }
    }
}

/// yt-dlp with no console window and no input, ended if Raygun lets go of it.
fn command(program: &Path) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW: no console flashes up for each run.
        command.creation_flags(0x0800_0000);
    }
    command
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// A program on the PATH, by name.
fn on_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(exe(name)))
        .find(|path| path.is_file())
}

/// Writes a fetched program beside its name and moves it there, runnable.
async fn write_tool(target: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| error.to_string())?;
    }
    let partial = target.with_extension("part");
    tokio::fs::write(&partial, bytes)
        .await
        .map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755))
            .await
            .map_err(|error| error.to_string())?;
    }
    tokio::fs::rename(&partial, target)
        .await
        .map_err(|error| error.to_string())
}

/// `ffmpeg.exe` and `ffprobe.exe` from the build's `bin/` folder into `tools`.
fn unpack_ffmpeg(bytes: &[u8], tools: &Path) -> Result<(), String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(tools).map_err(|error| error.to_string())?;
    let mut found = 0;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry
            .name()
            .map_err(|error| error.to_string())?
            .replace('\\', "/");
        let Some(file) = ["ffmpeg.exe", "ffprobe.exe"]
            .into_iter()
            .find(|file| name.ends_with(&format!("/bin/{file}")))
        else {
            continue;
        };
        let partial = tools.join(format!("{file}.part"));
        let mut out = std::fs::File::create(&partial).map_err(|error| error.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|error| error.to_string())?;
        drop(out);
        std::fs::rename(&partial, tools.join(file)).map_err(|error| error.to_string())?;
        found += 1;
    }
    if found == 0 {
        return Err("The FFmpeg build has no ffmpeg.exe".into());
    }
    Ok(())
}

/// The checksum a `SHA2-256SUMS`-style file gives `name` (`<hash>  <name>` lines).
fn checksum_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| hash.to_ascii_lowercase())
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// "downloaded total estimate" as a share, from whichever size yt-dlp knows.
fn progress_share(numbers: &str) -> Option<f64> {
    let mut parts = numbers
        .split_whitespace()
        .map(|part| part.parse::<f64>().ok());
    let done = parts.next().flatten()?;
    let total = parts.next().flatten().or_else(|| parts.next().flatten())?;
    (total > 0.0).then(|| (done / total).clamp(0.0, 1.0))
}

/// The bytes a progress line says were saved, and the size (exact, else yt-dlp's estimate).
fn progress_bytes(numbers: &str) -> Option<(u64, Option<u64>)> {
    let mut parts = numbers
        .split_whitespace()
        .map(|part| part.parse::<f64>().ok());
    let done = parts.next().flatten()?;
    let total = parts
        .next()
        .flatten()
        .or_else(|| parts.next().flatten())
        .filter(|total| *total > 0.0);
    Some((done.max(0.0) as u64, total.map(|total| total as u64)))
}

fn shown_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn watch_url(video_id: &str) -> String {
    format!("https://www.youtube.com/watch?v={video_id}")
}

/// A YouTube address's video, playlist or channel (M46.47): `watch?v=`, `youtu.be/`, `shorts/`,
/// `live/`, `embed/`, YouTube Music's, `playlist?list=` and `channel/UC…`.
pub fn parse_link(text: &str) -> Option<YouTubeLink> {
    let text = text.trim();
    let with_scheme = if text.starts_with("http://") || text.starts_with("https://") {
        text.to_owned()
    } else {
        format!("https://{text}")
    };
    let url = url::Url::parse(&with_scheme).ok()?;
    let host = url
        .host_str()?
        .trim_start_matches("www.")
        .trim_start_matches("m.");
    let query = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    let valid = |id: &str, length: std::ops::RangeInclusive<usize>| {
        length.contains(&id.len())
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    };
    let video = |id: String| {
        valid(&id, 11..=11).then(|| YouTubeLink {
            kind: YouTubeLinkKind::Video,
            url: watch_url(&id),
            id,
            title: None,
            author: None,
            thumbnail_url: None,
        })
    };
    match host {
        "youtu.be" => video(segments.first()?.to_string()),
        "youtube.com" | "music.youtube.com" | "youtube-nocookie.com" => match segments.as_slice() {
            ["watch"] => video(query("v")?),
            ["shorts" | "live" | "embed" | "v", id, ..] => video((*id).to_owned()),
            ["playlist"] => {
                let id = query("list")?;
                valid(&id, 10..=64).then(|| YouTubeLink {
                    kind: YouTubeLinkKind::Playlist,
                    url: format!("https://www.youtube.com/playlist?list={id}"),
                    id,
                    title: None,
                    author: None,
                    thumbnail_url: None,
                })
            }
            ["channel", id, ..] if id.starts_with("UC") => {
                valid(id, 24..=24).then(|| YouTubeLink {
                    kind: YouTubeLinkKind::Channel,
                    url: format!("https://www.youtube.com/channel/{id}"),
                    id: (*id).to_owned(),
                    title: None,
                    author: None,
                    thumbnail_url: None,
                })
            }
            _ => None,
        },
        _ => None,
    }
}

/// A video's title, channel and thumbnail from YouTube's oEmbed answer (no key needed).
pub async fn describe(client: &reqwest::Client, mut link: YouTubeLink) -> YouTubeLink {
    if link.kind == YouTubeLinkKind::Channel {
        return link;
    }
    #[derive(serde::Deserialize)]
    struct OEmbed {
        title: Option<String>,
        author_name: Option<String>,
        thumbnail_url: Option<String>,
    }
    let address = format!(
        "https://www.youtube.com/oembed?format=json&url={}",
        url::form_urlencoded::byte_serialize(link.url.as_bytes()).collect::<String>()
    );
    let answer = client
        .get(&address)
        .timeout(Duration::from_secs(10))
        .send()
        .await;
    if let Ok(response) = answer
        && response.status().is_success()
        && let Ok(found) = response.json::<OEmbed>().await
    {
        link.title = found.title;
        link.author = found.author_name;
        link.thumbnail_url = found.thumbnail_url;
    }
    if link.kind == YouTubeLinkKind::Video && link.thumbnail_url.is_none() {
        link.thumbnail_url = Some(format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", link.id));
    }
    link
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_addresses_of_every_shape_are_read() {
        for text in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42",
            "youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ?si=abc",
            "https://m.youtube.com/shorts/dQw4w9WgXcQ",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ&list=RDAMVM",
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
        ] {
            let link = parse_link(text).unwrap_or_else(|| panic!("{text}"));
            assert_eq!(link.kind, YouTubeLinkKind::Video, "{text}");
            assert_eq!(link.url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        }
        let list =
            parse_link("https://www.youtube.com/playlist?list=PLx0sYbCqOb8TBPRdmBHs5Iftvv9TPboYG")
                .unwrap();
        assert_eq!(list.kind, YouTubeLinkKind::Playlist);
        let channel =
            parse_link("https://www.youtube.com/channel/UCuAXFkgsw1L7xaCfnd5JJOw").unwrap();
        assert_eq!(channel.kind, YouTubeLinkKind::Channel);
        for text in [
            "daft punk",
            "https://example.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=short",
            "https://www.youtube.com/@someone",
        ] {
            assert!(parse_link(text).is_none(), "{text}");
        }
    }

    #[test]
    fn checksums_and_progress_are_read() {
        let sums = "abc123  yt-dlp\nDEF456  yt-dlp.exe\n789  *yt-dlp_linux\n";
        assert_eq!(checksum_for(sums, "yt-dlp.exe").as_deref(), Some("def456"));
        assert_eq!(checksum_for(sums, "yt-dlp_linux").as_deref(), Some("789"));
        assert_eq!(checksum_for(sums, "yt-dlp_macos"), None);
        assert_eq!(progress_share("50 200 NA"), Some(0.25));
        assert_eq!(progress_share("50 NA 100"), Some(0.5));
        assert_eq!(progress_share("50 NA NA"), None);
        assert_eq!(progress_bytes("50 200 NA"), Some((50, Some(200))));
        assert_eq!(progress_bytes("50 NA 100.5"), Some((50, Some(100))));
        assert_eq!(progress_bytes("50 NA NA"), Some((50, None)));
        assert_eq!(progress_bytes("NA NA NA"), None);
    }
}
