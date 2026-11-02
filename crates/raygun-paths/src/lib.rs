//! Resolves where Raygun keeps its data.
//!
//! This is the only place allowed to decide filesystem locations (ARCHITECTURE §3).
//! Desktop builds are portable: everything lives in a `raygun-data` folder next to the
//! executable, so the app can be moved or carried on a USB stick. Mobile builds use the
//! app sandbox. The crate has no Tauri dependency; the shell gathers an [`Environment`]
//! and hands it over, which keeps every branch unit-testable.

mod paths;

use std::fs;
use std::path::{Path, PathBuf};

pub use paths::{CacheKind, Paths};

/// Folder created next to the executable in portable mode.
pub const DATA_DIR_NAME: &str = "raygun-data";

/// Marker file in the system folder recording that the user picked installed mode.
pub const INSTALLED_MARKER: &str = "installed-mode";

/// How the data root was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    /// Next to the executable.
    Portable,
    /// OS app-data folder, chosen by the user because the portable folder wasn't writable.
    Installed,
    /// App sandbox on Android/iOS.
    Mobile,
    /// Set explicitly through `RAYGUN_HOME`.
    Override,
}

/// Why the user has to decide where data goes before the app can start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceReason {
    /// The folder next to the executable can't be written (e.g. under `Program Files`).
    Unwritable,
    /// macOS runs the app from a randomized read-only copy because it is quarantined.
    Translocated,
}

/// Everything resolution depends on, gathered by the shell at startup.
#[derive(Debug, Clone)]
pub struct Environment {
    /// Target OS as in `std::env::consts::OS`.
    pub os: String,
    /// Path of the running executable.
    pub exe: PathBuf,
    /// Value of `RAYGUN_HOME`, if set.
    pub raygun_home: Option<PathBuf>,
    /// Value of `APPIMAGE` (set by the AppImage runtime on Linux), if set.
    pub appimage: Option<PathBuf>,
    /// OS-provided per-app folder: the sandbox on mobile, the installed-mode location on desktop.
    pub system_dir: Option<PathBuf>,
}

impl Environment {
    /// Reads the process environment. `system_dir` comes from the platform layer.
    pub fn from_process(system_dir: Option<PathBuf>) -> std::io::Result<Self> {
        Ok(Self {
            os: std::env::consts::OS.to_owned(),
            exe: std::env::current_exe()?,
            raygun_home: non_empty_env("RAYGUN_HOME"),
            appimage: non_empty_env("APPIMAGE"),
            system_dir,
        })
    }

    fn is_mobile(&self) -> bool {
        matches!(self.os.as_str(), "android" | "ios")
    }
}

/// Outcome of [`resolve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Ready(Paths),
    /// No usable location without asking the user. `portable_root` is where data would have gone.
    NeedsChoice {
        reason: ChoiceReason,
        portable_root: PathBuf,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResolveError {
    /// Mobile builds and installed mode need the OS app folder, but none was provided.
    MissingSystemDir,
    /// The executable path has no parent folder.
    NoExecutableDir,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSystemDir => f.write_str("the OS app data folder is unavailable"),
            Self::NoExecutableDir => f.write_str("the executable has no parent folder"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Decides the data root. See `docs/phases/phase-1/M02-portable-paths.md` for the order of rules.
pub fn resolve(env: &Environment) -> Result<Resolution, ResolveError> {
    if let Some(home) = &env.raygun_home {
        return Ok(Resolution::Ready(Paths::new(
            home.clone(),
            StorageMode::Override,
        )));
    }

    if env.is_mobile() {
        let sandbox = env
            .system_dir
            .clone()
            .ok_or(ResolveError::MissingSystemDir)?;
        return Ok(Resolution::Ready(Paths::new(sandbox, StorageMode::Mobile)));
    }

    let portable_root = portable_base(env)?.join(DATA_DIR_NAME);

    if env.os == "macos" && is_translocated(&env.exe) {
        return Ok(Resolution::NeedsChoice {
            reason: ChoiceReason::Translocated,
            portable_root,
        });
    }

    if portable_root.is_dir() && is_writable(&portable_root) {
        return Ok(Resolution::Ready(Paths::new(
            portable_root,
            StorageMode::Portable,
        )));
    }

    if let Some(system_dir) = &env.system_dir
        && system_dir.join(INSTALLED_MARKER).is_file()
    {
        return Ok(Resolution::Ready(Paths::new(
            system_dir.clone(),
            StorageMode::Installed,
        )));
    }

    if is_writable(&portable_root) {
        return Ok(Resolution::Ready(Paths::new(
            portable_root,
            StorageMode::Portable,
        )));
    }

    Ok(Resolution::NeedsChoice {
        reason: ChoiceReason::Unwritable,
        portable_root,
    })
}

/// Records the user's choice of installed mode so later launches skip the question.
pub fn choose_installed(env: &Environment) -> Result<Paths, ChooseError> {
    let system_dir = env
        .system_dir
        .clone()
        .ok_or(ChooseError::Resolve(ResolveError::MissingSystemDir))?;
    fs::create_dir_all(&system_dir).map_err(ChooseError::Io)?;
    fs::write(
        system_dir.join(INSTALLED_MARKER),
        "Raygun stores its data here because its own folder was not writable.\n",
    )
    .map_err(ChooseError::Io)?;
    Ok(Paths::new(system_dir, StorageMode::Installed))
}

#[derive(Debug)]
pub enum ChooseError {
    Resolve(ResolveError),
    Io(std::io::Error),
}

impl std::fmt::Display for ChooseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resolve(error) => error.fmt(f),
            Self::Io(error) => write!(f, "could not write the installed-mode marker: {error}"),
        }
    }
}

impl std::error::Error for ChooseError {}

/// Folder that holds the user-visible app, i.e. where `raygun-data` belongs.
fn portable_base(env: &Environment) -> Result<PathBuf, ResolveError> {
    if env.os == "linux"
        && let Some(appimage) = &env.appimage
    {
        return appimage
            .parent()
            .map(Path::to_path_buf)
            .ok_or(ResolveError::NoExecutableDir);
    }

    if env.os == "macos"
        && let Some(bundle) = env
            .exe
            .ancestors()
            .find(|ancestor| ancestor.extension().is_some_and(|ext| ext == "app"))
    {
        return bundle
            .parent()
            .map(Path::to_path_buf)
            .ok_or(ResolveError::NoExecutableDir);
    }

    env.exe
        .parent()
        .map(Path::to_path_buf)
        .ok_or(ResolveError::NoExecutableDir)
}

fn is_translocated(exe: &Path) -> bool {
    exe.components()
        .any(|component| component.as_os_str() == "AppTranslocation")
}

/// Creates `dir` if needed and checks that a file can be written into it.
fn is_writable(dir: &Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(format!(".write-probe-{}", std::process::id()));
    let writable = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .is_ok();
    let _ = fs::remove_file(&probe);
    writable
}

fn non_empty_env(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests;

/// The user's Downloads folder on desktop, where exported files go (M46.6). Raygun's own data
/// stays in its data root; this is only for files the user saves, as a browser's downloads.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn user_downloads() -> Option<PathBuf> {
    dirs::download_dir()
}

/// The user's Pictures folder on desktop, where saved pages and screenshots go (M46.6).
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn user_pictures() -> Option<PathBuf> {
    dirs::picture_dir()
}
