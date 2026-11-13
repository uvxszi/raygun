//! yt-dlp (M46.47): YouTube videos and their sound saved on desktop, and the songs whose YouTube
//! audio file couldn't be downloaded (M46.51) saved as files.
//!
//! yt-dlp is looked for in `<data root>/tools/`, then on the PATH. When it's missing it is fetched
//! from its GitHub releases the first time it's needed (the build for this system and processor,
//! else the zipapp that runs on the system's Python) and checked against the release's
//! `SHA2-256SUMS`. FFmpeg (which joins YouTube's separate picture and sound above 360p) is used
//! when found the same way, and on Windows and Linux (x64, ARM64; Windows x86 too) it can be
//! fetched from yt-dlp's FFmpeg builds. Phones have neither: everything here says it isn't
//! supported there.
//!
//! Videos and sound go to the user's `Downloads/Raygun/YouTube/`, one at a time, listed in
//! Downloads → Exports. yt-dlp runs without a console window and is stopped with the job.
//!
//! A fetch of yt-dlp or FFmpeg can be cancelled ([`YtDlpService::cancel_install`]): it stops
//! wherever it is, and nothing of it is kept.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use raygun_model::{YouTubeLink, YouTubeLinkKind, YtDlpJob, YtDlpKind, YtDlpState, YtDlpStatus};
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::events::YtDlpEvent;

const RELEASES: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
const FFMPEG_RELEASES: &str = "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest";
/// yt-dlp's own file for this system and processor; elsewhere (riscv64, the BSDs) the zipapp,
/// which runs on the system's Python 3.
const ASSET: &str = if cfg!(windows) {
    if cfg!(target_arch = "aarch64") {
        "yt-dlp_arm64.exe"
    } else if cfg!(target_arch = "x86") {
        "yt-dlp_x86.exe"
    } else {
        "yt-dlp.exe"
    }
} else if cfg!(target_os = "macos") {
    "yt-dlp_macos"
} else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    "yt-dlp_linux"
} else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
    "yt-dlp_linux_aarch64"
} else {
    "yt-dlp"
};
/// yt-dlp's FFmpeg build for this system and processor, when there's one.
const FFMPEG_ARCHIVE: Option<&str> = if cfg!(all(windows, target_arch = "x86_64")) {
    Some(FFMPEG_WIN64)
} else if cfg!(all(windows, target_arch = "x86")) {
    Some("ffmpeg-master-latest-win32-gpl.zip")
} else if cfg!(all(windows, target_arch = "aarch64")) {
    Some("ffmpeg-master-latest-winarm64-gpl.zip")
} else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    Some(FFMPEG_LINUX64)
} else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
    Some("ffmpeg-master-latest-linuxarm64-gpl.tar.xz")
} else {
    None
};
const FFMPEG_WIN64: &str = "ffmpeg-master-latest-win64-gpl.zip";
const FFMPEG_LINUX64: &str = "ffmpeg-master-latest-linux64-gpl.tar.xz";
/// The biggest yt-dlp or FFmpeg download accepted.
const MAX_TOOL_BYTES: u64 = 400 << 20;
/// A song that takes longer than this has failed.
const SONG_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Finished jobs kept in the list.
const KEPT_JOBS: usize = 40;
/// How often a running download's progress is announced at most.
const PROGRESS_EVERY: Duration = Duration::from_millis(400);
/// Marks Raygun's own lines among yt-dlp's output.
const PROGRESS: &str = "RAYGUN_PROGRESS";
const FILE: &str = "RAYGUN_FILE";
/// How often a job waiting for FFmpeg checks whether it was stopped.
const STOP_CHECK: Duration = Duration::from_millis(250);

/// Whether yt-dlp can run here: desktop only.
pub const fn supported() -> bool {
    !cfg!(any(target_os = "android", target_os = "ios"))
}

/// What stopped a fetch of yt-dlp or FFmpeg: its Cancel, or a failure (why).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    Cancelled,
    Failed(String),
}

impl ToolError {
    /// What a caller that needed `tool` says.
    pub fn message(self, tool: &str) -> String {
        match self {
            ToolError::Cancelled => format!("Getting {tool} was cancelled"),
            ToolError::Failed(why) => why,
        }
    }
}

impl From<String> for ToolError {
    fn from(why: String) -> Self {
        ToolError::Failed(why)
    }
}

/// The tools Raygun fetches here.
#[derive(Debug, Clone, Copy)]
enum Tool {
    YtDlp,
    Ffmpeg,
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
    /// How far a fetch is, when its size is known.
    progress: Option<f64>,
    message: Option<String>,
    /// Stops the fetch running now.
    cancel: Option<CancellationToken>,
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
            progress: found.installing.then_some(found.progress).flatten(),
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

    /// FFmpeg and ffprobe for converting videos (desktop): Raygun's own, else a pair on the PATH.
    pub fn ffmpeg_tools(&self) -> Option<crate::export::convert::ffmpeg::Tools> {
        if !supported() {
            return None;
        }
        use crate::export::convert::ffmpeg::Tools;
        Tools::in_dir(&self.tools).or_else(|| {
            let dir = on_path("ffmpeg")?.parent()?.to_path_buf();
            Tools::in_dir(&dir)
        })
    }

    /// yt-dlp, fetched first when it isn't here.
    async fn ready(&self) -> Result<PathBuf, String> {
        // yt-dlp goes to the network on its own: it doesn't run while Offline Mode is on.
        raygun_net::network_mode::check_online()?;
        if !supported() {
            return Err("YouTube downloads need Raygun on a computer".into());
        }
        if let Some((path, _)) = self.program().await {
            return Ok(path);
        }
        self.fetch_tool(Tool::YtDlp)
            .await
            .map_err(|error| error.message("yt-dlp"))?;
        self.program()
            .await
            .map(|(path, _)| path)
            .ok_or_else(|| "yt-dlp was fetched but doesn't start".into())
    }

    /// Fetches yt-dlp's latest release into Raygun's tools, checked against its checksums. A
    /// fetch cancelled ([`Self::cancel_install`]) answers with the status as it was.
    pub async fn install(&self) -> Result<YtDlpStatus, String> {
        if !supported() {
            return Err("yt-dlp runs on computers only".into());
        }
        match self.fetch_tool(Tool::YtDlp).await {
            Ok(()) | Err(ToolError::Cancelled) => Ok(self.status().await),
            Err(ToolError::Failed(why)) => Err(why),
        }
    }

    async fn fetch_ytdlp(&self, cancel: &CancellationToken) -> Result<(), ToolError> {
        let sums = download(
            &self.client,
            &format!("{RELEASES}/SHA2-256SUMS"),
            cancel,
            |_| {},
        )
        .await?;
        let sums = String::from_utf8(sums).map_err(|error| error.to_string())?;
        let expected = checksum_for(&sums, ASSET)
            .ok_or_else(|| format!("yt-dlp's checksums don't list {ASSET}"))?;
        let bytes = download(
            &self.client,
            &format!("{RELEASES}/{ASSET}"),
            cancel,
            self.announce_share(),
        )
        .await?;
        if sha256_hex(&bytes) != expected {
            return Err(ToolError::Failed(
                "yt-dlp's download doesn't match its checksum".into(),
            ));
        }
        // Cancelled after its last byte: nothing is written.
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        let target = self.tools.join(exe("yt-dlp"));
        write_tool(&target, &bytes).await?;
        self.found.lock().expect("yt-dlp state poisoned").program = None;
        tracing::info!(path = %target.display(), "yt-dlp installed");
        Ok(())
    }

    /// Fetches FFmpeg (yt-dlp's builds) for videos above 360p. A fetch cancelled
    /// ([`Self::cancel_install`]) answers with the status as it was.
    pub async fn install_ffmpeg(&self) -> Result<YtDlpStatus, String> {
        match self.fetch_ffmpeg().await {
            Ok(()) | Err(ToolError::Cancelled) => Ok(self.status().await),
            Err(ToolError::Failed(why)) => Err(why),
        }
    }

    /// Fetches FFmpeg into Raygun's tools where yt-dlp builds it for this computer; elsewhere
    /// (macOS, the BSDs) the system's package manager has it.
    pub async fn fetch_ffmpeg(&self) -> Result<(), ToolError> {
        if FFMPEG_ARCHIVE.is_none() {
            return Err(ToolError::Failed(
                "Install FFmpeg with your system's package manager".into(),
            ));
        }
        self.fetch_tool(Tool::Ffmpeg).await
    }

    /// [`Self::fetch_ffmpeg`] for a job that can be stopped: `stop` turning true (its Cancel)
    /// cancels the fetch.
    pub async fn fetch_ffmpeg_for(&self, stop: &AtomicBool) -> Result<(), ToolError> {
        let fetch = self.fetch_ffmpeg();
        tokio::pin!(fetch);
        loop {
            tokio::select! {
                result = &mut fetch => return result,
                () = tokio::time::sleep(STOP_CHECK) => {
                    if stop.load(Ordering::SeqCst) {
                        self.cancel_install();
                    }
                }
            }
        }
    }

    /// Stops the fetch of yt-dlp or FFmpeg running now: the transfer ends at once and nothing of
    /// it is kept. Whether one was running.
    pub fn cancel_install(&self) -> bool {
        let found = self.found.lock().expect("yt-dlp state poisoned");
        let Some(cancel) = &found.cancel else {
            return false;
        };
        cancel.cancel();
        true
    }

    /// One fetch at a time, shown in [`Self::status`] while it runs; a failure's reason stays
    /// there, a cancel leaves none.
    async fn fetch_tool(&self, tool: Tool) -> Result<(), ToolError> {
        let _turn = self.install_turn.lock().await;
        let cancel = CancellationToken::new();
        self.set_installing(true, None, Some(cancel.clone()));
        let result = match tool {
            Tool::YtDlp => self.fetch_ytdlp(&cancel).await,
            Tool::Ffmpeg => {
                fetch_ffmpeg_into(&self.client, &self.tools, &cancel, self.announce_share()).await
            }
        };
        let message = match &result {
            Err(ToolError::Failed(why)) => Some(why.clone()),
            Ok(()) | Err(ToolError::Cancelled) => None,
        };
        if result == Err(ToolError::Cancelled) {
            tracing::info!(?tool, "fetching a tool was cancelled");
        }
        self.set_installing(false, message, None);
        result
    }

    /// Shows how far a fetch is, at most every [`PROGRESS_EVERY`].
    fn announce_share(&self) -> impl Fn(f64) + '_ {
        let shown = Mutex::new(None::<Instant>);
        move |share| {
            let mut shown = shown.lock().expect("progress poisoned");
            if shown.is_some_and(|at| at.elapsed() < PROGRESS_EVERY) {
                return;
            }
            *shown = Some(Instant::now());
            self.found.lock().expect("yt-dlp state poisoned").progress = Some(share);
            let _ = YtDlpEvent(String::new()).emit(&self.app);
        }
    }

    fn set_installing(
        &self,
        installing: bool,
        message: Option<String>,
        cancel: Option<CancellationToken>,
    ) {
        {
            let mut found = self.found.lock().expect("yt-dlp state poisoned");
            found.installing = installing;
            found.progress = None;
            found.message = message;
            found.cancel = cancel;
        }
        let _ = YtDlpEvent(String::new()).emit(&self.app);
    }

    // ---- Songs ----

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
        raygun_net::network_mode::check_online()?;
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
        // A video above 360p is two downloads (picture, then sound) that FFmpeg joins.
        let parts = if kind == YtDlpKind::Video && ffmpeg.is_some() {
            2
        } else {
            1
        };
        let overall = Mutex::new(Overall::new(parts));
        run(command, cancel, move |numbers| {
            let Some(share) = progress_share(numbers) else {
                return;
            };
            // yt-dlp reports every block: the list hears each whole percent, never backwards.
            let shown = overall
                .lock()
                .expect("yt-dlp progress poisoned")
                .next(share);
            if let Some(progress) = shown {
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

    // ---- YouTube for extensions ----

    /// Runs yt-dlp with `args` (fetched first when it isn't here) and hands back what it printed.
    pub async fn print(self: &Arc<Self>, args: Vec<String>) -> Result<String, String> {
        // Fetched apart from the caller, so a lookup that stops waiting doesn't stop the fetch.
        let service = self.clone();
        let program = tauri::async_runtime::spawn(async move { service.ready().await })
            .await
            .map_err(|error| error.to_string())??;
        let output = command(&program)
            .args(&args)
            .output()
            .await
            .map_err(|error| format!("yt-dlp didn't start: {error}"))?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let errors = String::from_utf8_lossy(&output.stderr);
        Err(errors
            .lines()
            .filter_map(|line| line.trim().strip_prefix("ERROR:"))
            .next_back()
            .map(|line| line.trim().to_owned())
            .unwrap_or_else(|| format!("yt-dlp stopped ({})", output.status)))
    }
}

/// Lets extensions' `raygun.youtube` (converted Nuclear plugins' `Ytdlp`) ask yt-dlp what
/// YouTube's own API didn't answer: desktop only.
pub fn answer_for_extensions(service: &Arc<YtDlpService>) {
    if !supported() {
        return;
    }
    let service = Arc::downgrade(service);
    raygun_ext_host::youtube::set_ytdlp(Arc::new(move |args| {
        let service = service.clone();
        Box::pin(async move {
            let service = service.upgrade().ok_or("yt-dlp has stopped")?;
            service.print(args).await
        })
    }));
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

/// Fetches yt-dlp's build of FFmpeg for this computer into `tools` (`ffmpeg` and `ffprobe`),
/// checked against the release's checksums; `progress` hears the share of the archive fetched,
/// and `cancel` stops it with nothing written.
pub(crate) async fn fetch_ffmpeg_into(
    client: &reqwest::Client,
    tools: &Path,
    cancel: &CancellationToken,
    progress: impl Fn(f64),
) -> Result<(), ToolError> {
    let archive = FFMPEG_ARCHIVE
        .ok_or_else(|| "yt-dlp builds no FFmpeg for this kind of computer".to_owned())?;
    fetch_ffmpeg_from(client, FFMPEG_RELEASES, archive, tools, cancel, progress).await
}

/// [`fetch_ffmpeg_into`] from the release at `releases`, its build `archive` (a Windows `.zip` or
/// a Linux `.tar.xz`).
async fn fetch_ffmpeg_from(
    client: &reqwest::Client,
    releases: &str,
    archive: &str,
    tools: &Path,
    cancel: &CancellationToken,
    progress: impl Fn(f64),
) -> Result<(), ToolError> {
    let sums = download(
        client,
        &format!("{releases}/checksums.sha256"),
        cancel,
        |_| {},
    )
    .await?;
    let sums = String::from_utf8(sums).map_err(|error| error.to_string())?;
    let expected = checksum_for(&sums, archive)
        .ok_or_else(|| format!("FFmpeg's checksums don't list {archive}"))?;
    let bytes = download(client, &format!("{releases}/{archive}"), cancel, progress).await?;
    if sha256_hex(&bytes) != expected {
        return Err(ToolError::Failed(
            "FFmpeg's download doesn't match its checksum".into(),
        ));
    }
    // Cancelled after its last byte: nothing is unpacked.
    if cancel.is_cancelled() {
        return Err(ToolError::Cancelled);
    }
    let tools = tools.to_owned();
    let zip = archive.ends_with(".zip");
    tokio::task::spawn_blocking(move || unpack_ffmpeg(zip, &bytes, &tools))
        .await
        .map_err(|error| error.to_string())??;
    tracing::info!("FFmpeg installed");
    Ok(())
}

/// A tool's file, kept in memory as it comes so `progress` hears how far it is (when its size is
/// known); `cancel` stops it wherever it is (waiting for the answer, between two pieces).
async fn download(
    client: &reqwest::Client,
    url: &str,
    cancel: &CancellationToken,
    progress: impl Fn(f64),
) -> Result<Vec<u8>, ToolError> {
    let sent = client.get(url).timeout(Duration::from_secs(900)).send();
    let mut response = tokio::select! {
        biased;
        () = cancel.cancelled() => return Err(ToolError::Cancelled),
        response = sent => response.map_err(|error| format!("{url}: {error}"))?,
    };
    if !response.status().is_success() {
        return Err(format!("{url}: HTTP {}", response.status().as_u16()).into());
    }
    let size = response.content_length();
    if size.is_some_and(|size| size > MAX_TOOL_BYTES) {
        return Err(format!("{url} is too big").into());
    }
    let mut bytes = Vec::with_capacity(size.unwrap_or(0) as usize);
    loop {
        let chunk = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(ToolError::Cancelled),
            chunk = response.chunk() => chunk.map_err(|error| format!("{url}: {error}"))?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        bytes.extend_from_slice(&chunk);
        if bytes.len() as u64 > MAX_TOOL_BYTES {
            return Err(format!("{url} is too big").into());
        }
        if let Some(size) = size.filter(|size| *size > 0) {
            progress((bytes.len() as f64 / size as f64).min(1.0));
        }
    }
    Ok(bytes)
}

/// `ffmpeg` and `ffprobe` from the build's `bin/` folder into `tools`: a Windows build's `.exe`s
/// from its zip (`zip`), else a Linux build's programs from its `.tar.xz`, made runnable.
fn unpack_ffmpeg(zip: bool, bytes: &[u8], tools: &Path) -> Result<(), String> {
    std::fs::create_dir_all(tools).map_err(|error| error.to_string())?;
    let found = if zip {
        unpack_ffmpeg_zip(bytes, tools)?
    } else {
        unpack_ffmpeg_tar_xz(bytes, tools)?
    };
    if found == 0 {
        return Err("The FFmpeg build has no ffmpeg".into());
    }
    Ok(())
}

/// The program `name` (a path in the archive) is, when it's one of the build's `bin/` programs.
fn ffmpeg_program(name: &str, files: [&'static str; 2]) -> Option<&'static str> {
    let name = name.replace('\\', "/");
    files
        .into_iter()
        .find(|file| name.ends_with(&format!("/bin/{file}")))
}

fn unpack_ffmpeg_zip(bytes: &[u8], tools: &Path) -> Result<usize, String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| error.to_string())?;
    let mut found = 0;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry.name().map_err(|error| error.to_string())?.into_owned();
        let Some(file) = ffmpeg_program(&name, ["ffmpeg.exe", "ffprobe.exe"]) else {
            continue;
        };
        write_program(&mut entry, &tools.join(file))?;
        found += 1;
    }
    Ok(found)
}

fn unpack_ffmpeg_tar_xz(bytes: &[u8], tools: &Path) -> Result<usize, String> {
    let mut archive = tar::Archive::new(xz::read::XzDecoder::new(std::io::Cursor::new(bytes)));
    let mut found = 0;
    for entry in archive.entries().map_err(|error| error.to_string())? {
        let mut entry = entry.map_err(|error| error.to_string())?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let name = entry
            .path()
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .into_owned();
        let Some(file) = ffmpeg_program(&name, ["ffmpeg", "ffprobe"]) else {
            continue;
        };
        write_program(&mut entry, &tools.join(file))?;
        found += 1;
    }
    Ok(found)
}

/// Writes a program beside `target` and moves it there, runnable.
fn write_program(content: &mut impl std::io::Read, target: &Path) -> Result<(), String> {
    let partial = target.with_extension("part");
    let mut out = std::fs::File::create(&partial).map_err(|error| error.to_string())?;
    std::io::copy(content, &mut out).map_err(|error| error.to_string())?;
    drop(out);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| error.to_string())?;
    }
    std::fs::rename(&partial, target).map_err(|error| error.to_string())
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

/// yt-dlp's progress through each file it fetches (each going 0 → 1) as one share of the whole
/// download that only moves forward, given when it moved a percent and a moment has passed.
struct Overall {
    parts: u32,
    part: u32,
    last: f64,
    shown: f64,
    announced: Option<Instant>,
}

impl Overall {
    fn new(parts: u32) -> Self {
        Self {
            parts: parts.max(1),
            part: 0,
            last: 0.0,
            shown: 0.0,
            announced: None,
        }
    }

    fn next(&mut self, share: f64) -> Option<f64> {
        // Starting again from the bottom: the next file.
        if share + 0.5 < self.last && self.part + 1 < self.parts {
            self.part += 1;
        }
        self.last = share;
        let overall = ((f64::from(self.part) + share) / f64::from(self.parts)).clamp(0.0, 1.0);
        if overall < self.shown + 0.01 {
            return None;
        }
        if self
            .announced
            .is_some_and(|at| at.elapsed() < PROGRESS_EVERY)
            && overall < 1.0
        {
            return None;
        }
        self.shown = overall;
        self.announced = Some(Instant::now());
        Some(overall)
    }
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
    // Phone keyboards capitalize a field's first letter: "Https://…" is still an address.
    let has_scheme = |scheme: &str| {
        text.get(..scheme.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
    };
    let with_scheme = if has_scheme("http://") || has_scheme("https://") {
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
            "Https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "HTTPS://YouTu.be/dQw4w9WgXcQ",
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

    #[test]
    fn progress_of_a_picture_and_its_sound_only_moves_forward() {
        let mut overall = Overall::new(2);
        assert_eq!(overall.next(0.5), Some(0.25));
        // Too soon after the last one, and too small a step: not announced.
        assert_eq!(overall.next(0.51), None);
        overall.announced = None;
        assert_eq!(overall.next(1.0), Some(0.5));
        // The sound starts from zero: the whole goes on from half.
        overall.announced = None;
        assert_eq!(overall.next(0.1), Some(0.55));
        overall.announced = None;
        assert_eq!(overall.next(1.0), Some(1.0));

        // One file: a second drop doesn't go past the end.
        let mut single = Overall::new(1);
        assert_eq!(single.next(0.8), Some(0.8));
        single.announced = None;
        assert_eq!(single.next(0.1), None, "never backwards");
    }

    /// A zip laid out as yt-dlp's FFmpeg builds are.
    fn ffmpeg_zip() -> Vec<u8> {
        use std::io::Write as _;
        let mut buffer = Vec::new();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
        let options = zip::write::SimpleFileOptions::default();
        for name in [
            "ffmpeg-x/bin/ffmpeg.exe",
            "ffmpeg-x/bin/ffprobe.exe",
            "ffmpeg-x/README",
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(name.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        buffer
    }

    /// A `.tar.xz` laid out as yt-dlp's Linux FFmpeg builds are.
    fn ffmpeg_tar_xz() -> Vec<u8> {
        let mut tar = tar::Builder::new(Vec::new());
        for name in [
            "ffmpeg-x/bin/ffmpeg",
            "ffmpeg-x/bin/ffprobe",
            "ffmpeg-x/bin/ffplay",
            "ffmpeg-x/doc/ffmpeg.html",
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(name.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, name, name.as_bytes())
                .unwrap();
        }
        let tar = tar.into_inner().unwrap();
        let mut xz = xz::write::XzEncoder::new(Vec::new(), 6);
        std::io::Write::write_all(&mut xz, &tar).unwrap();
        xz.finish().unwrap()
    }

    /// Releases at `/ok` (whole), `/slow` (the archive stops after a piece) and `/hang` (no
    /// answer at all).
    async fn serve_releases() -> String {
        use axum::Router;
        use axum::body::{Body, Bytes};
        use axum::routing::get;
        use futures_util::StreamExt as _;

        let zip = ffmpeg_zip();
        let tar_xz = ffmpeg_tar_xz();
        let sums = format!(
            "{}  {FFMPEG_WIN64}\n{}  {FFMPEG_LINUX64}\n",
            sha256_hex(&zip),
            sha256_hex(&tar_xz)
        );
        let app = Router::new()
            .route(
                "/ok/checksums.sha256",
                get({
                    let sums = sums.clone();
                    move || async move { sums }
                }),
            )
            .route(
                &format!("/ok/{FFMPEG_WIN64}"),
                get(move || async move { zip }),
            )
            .route(
                &format!("/ok/{FFMPEG_LINUX64}"),
                get(move || async move { tar_xz }),
            )
            .route("/slow/checksums.sha256", get(move || async move { sums }))
            .route(
                &format!("/slow/{FFMPEG_WIN64}"),
                get(|| async {
                    let piece = futures_util::stream::once(async {
                        Ok::<_, std::io::Error>(Bytes::from(vec![1u8; 100]))
                    });
                    Body::from_stream(piece.chain(futures_util::stream::pending()))
                }),
            )
            .route(
                "/hang/checksums.sha256",
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

    /// Cancels `cancel` in a moment.
    fn cancel_soon(cancel: &CancellationToken) {
        let cancel = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            cancel.cancel();
        });
    }

    #[tokio::test]
    async fn ffmpeg_fetches_check_out_and_a_cancel_keeps_nothing() {
        let base = serve_releases().await;
        let client = raygun_net::http_client().unwrap();

        // Windows builds come as zips, Linux builds as `.tar.xz`.
        for (archive, programs) in [
            (FFMPEG_WIN64, ["ffmpeg.exe", "ffprobe.exe"]),
            (FFMPEG_LINUX64, ["ffmpeg", "ffprobe"]),
        ] {
            let tools = tempfile::tempdir().unwrap();
            let fetched = fetch_ffmpeg_from(
                &client,
                &format!("{base}/ok"),
                archive,
                tools.path(),
                &CancellationToken::new(),
                |_| {},
            )
            .await;
            assert_eq!(fetched, Ok(()), "{archive}");
            let mut names: Vec<String> = std::fs::read_dir(tools.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            assert_eq!(names, programs, "{archive}");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(tools.path().join(programs[0]))
                    .unwrap()
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o111, 0o111, "{archive}: runnable");
            }
        }

        // Stopped halfway through the archive, and before the checksums even answered.
        for release in ["slow", "hang"] {
            let tools = tempfile::tempdir().unwrap();
            let cancel = CancellationToken::new();
            cancel_soon(&cancel);
            let stopped = tokio::time::timeout(
                Duration::from_secs(10),
                fetch_ffmpeg_from(
                    &client,
                    &format!("{base}/{release}"),
                    FFMPEG_WIN64,
                    tools.path(),
                    &cancel,
                    |_| {},
                ),
            )
            .await
            .expect("a cancel doesn't wait for the server");
            assert_eq!(stopped, Err(ToolError::Cancelled), "{release}");
            assert_eq!(
                std::fs::read_dir(tools.path()).unwrap().count(),
                0,
                "{release}: nothing kept"
            );
        }

        // A cancel before the start fetches nothing.
        let cancel = CancellationToken::new();
        cancel.cancel();
        let stopped = download(&client, &format!("{base}/ok/{FFMPEG_WIN64}"), &cancel, |_| {
            panic!("nothing arrives")
        })
        .await;
        assert_eq!(stopped, Err(ToolError::Cancelled));
        assert_eq!(
            ToolError::Cancelled.message("FFmpeg"),
            "Getting FFmpeg was cancelled"
        );
    }
}
