//! The files a Kodi add-on's Python sees: Kodi's paths, each mounted on one store.
//!
//! - `/kodi/home/addons/<id>`: the add-on and its libraries, read-only, from the extension's
//!   folder;
//! - `/kodi/home/userdata` and `/kodi/temp`: its own writable folders in the extension's data
//!   (each add-on's), within a quota;
//! - `/kodi/python/raygun` and `/kodi/python/stdlib`: Raygun's modules and the standard library,
//!   compiled in.
//!
//! Anything else doesn't exist. Paths are resolved lexically (`..` never leaves the root) and
//! each part checked, so no path reaches outside its store.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

/// Raygun's Python modules (`src/kodi/lib`).
pub(crate) const RAYGUN_ROOT: &str = "/kodi/python/raygun";
/// The standard library's Python modules.
pub(crate) const STDLIB_ROOT: &str = "/kodi/python/stdlib";
/// Add-ons, the plugin's own and its libraries.
pub(crate) const ADDONS_ROOT: &str = "/kodi/home/addons";
const USERDATA_ROOT: &str = "/kodi/home/userdata";
const TEMP_ROOT: &str = "/kodi/temp";
/// Largest file read or written at once.
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
/// Most an add-on may keep in its writable folders.
pub(crate) const QUOTA_BYTES: u64 = 256 * 1024 * 1024;

/// Raygun's modules shadowed by nothing: `xbmc*`, the OS stand-ins, `sqlite3`. The rest of
/// `lib/` ([`EXTRAS`]) comes after the add-on's own folders, so an add-on can bring its own.
pub(crate) const EXTRAS: &[&str] = &[
    "requests",
    "urllib3",
    "inputstreamhelper",
    "StorageServer",
    "imp",
    "distutils",
    "cgi",
];

macro_rules! raygun_lib {
    ($($path:literal),* $(,)?) => {
        &[$(($path, include_str!(concat!("../lib/", $path)))),*]
    };
}

/// `src/kodi/lib`, compiled in.
static RAYGUN_FILES: &[(&str, &str)] = raygun_lib![
    "_raygun_boot.py",
    "_raygun_files.py",
    "_raygun_http.py",
    "_raygun_kodi.py",
    "_raygun_posix.py",
    "StorageServer.py",
    "cgi.py",
    "ctypes/__init__.py",
    "ctypes/util.py",
    "distutils/__init__.py",
    "distutils/util.py",
    "distutils/version.py",
    "fcntl.py",
    "imp.py",
    "inputstreamhelper/__init__.py",
    "inputstreamhelper/api.py",
    "platform.py",
    "requests/__init__.py",
    "requests/adapters.py",
    "requests/auth.py",
    "requests/compat.py",
    "requests/cookies.py",
    "requests/exceptions.py",
    "requests/hooks.py",
    "requests/models.py",
    "requests/packages/__init__.py",
    "requests/sessions.py",
    "requests/status_codes.py",
    "requests/structures.py",
    "requests/utils.py",
    "select.py",
    "signal.py",
    "socket.py",
    "sqlite3/__init__.py",
    "sqlite3/dbapi2.py",
    "ssl.py",
    "subprocess.py",
    "urllib3/__init__.py",
    "urllib3/exceptions.py",
    "urllib3/util/__init__.py",
    "urllib3/util/retry.py",
    "urllib3/util/timeout.py",
    "urllib3/util/url.py",
    "xbmc.py",
    "xbmcaddon.py",
    "xbmcdrm.py",
    "xbmcgui.py",
    "xbmcplugin.py",
    "xbmcvfs.py",
];

/// The standard library's Python modules, packed at build time (`build.rs`).
static STDLIB_ZIP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kodi-stdlib.zip"));

/// The standard library's files by path, read from [`STDLIB_ZIP`] once.
fn stdlib() -> &'static BTreeMap<String, Vec<u8>> {
    static FILES: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    FILES.get_or_init(|| {
        let mut files = BTreeMap::new();
        let Ok(mut archive) = zip::ZipArchive::new(io::Cursor::new(STDLIB_ZIP)) else {
            return files;
        };
        for index in 0..archive.len() {
            let Ok(mut file) = archive.by_index(index) else {
                continue;
            };
            let name = file
                .name()
                .map(|name| name.into_owned())
                .unwrap_or_default();
            let mut contents = Vec::new();
            if file.read_to_end(&mut contents).is_ok() {
                files.insert(name, contents);
            }
        }
        files
    })
}

/// A mounted store.
#[derive(Debug, Clone)]
pub(crate) enum Store {
    /// A folder on disk.
    Disk { root: PathBuf, writable: bool },
    /// Raygun's modules.
    Raygun,
    /// The standard library.
    Stdlib,
    /// Files kept in memory (tests, and add-ons loaded without a data folder).
    Memory(Arc<Mutex<MemoryTree>>),
    /// The plugin's own folder: its files read-only (`lower`), what it writes there kept in its
    /// data (`upper`) and read first, as some plugins keep caches beside their code.
    Overlay { lower: PathBuf, upper: PathBuf },
}

#[derive(Debug, Default)]
pub(crate) struct MemoryTree {
    files: BTreeMap<String, (Vec<u8>, SystemTime)>,
    dirs: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct Mount {
    /// A virtual folder, without a trailing `/`.
    at: String,
    store: Store,
}

/// What a path names.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Stat {
    pub dir: bool,
    pub size: u64,
    /// Seconds since 1970.
    pub modified: f64,
}

/// What [`Files::find_module`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Found {
    Module(String),
    /// Its `__init__.py`, and the folder it searches.
    Package(String, String),
    /// Folders without an `__init__.py` (a namespace package).
    Namespace(Vec<String>),
}

fn not_found(path: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("No such file or directory: '{path}'"),
    )
}

fn read_only(path: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("Read-only file system: '{path}'"),
    )
}

/// A path with `.` and `..` resolved; `None` when it isn't absolute or a part can't be a name on
/// every system Raygun runs on.
pub(crate) fn normalize(path: &str) -> Option<String> {
    if !path.starts_with('/') || path.contains('\0') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => {
                if !safe_part(part) {
                    return None;
                }
                parts.push(part);
            }
        }
    }
    Some(format!("/{}", parts.join("/")))
}

/// A path part that stays one plain name on Android, Linux and Windows.
fn safe_part(part: &str) -> bool {
    if part.contains(['\\', ':', '*', '?', '"', '<', '>', '|']) || part.len() > 255 {
        return false;
    }
    if part.ends_with('.') || part.ends_with(' ') {
        return false;
    }
    let stem = part
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
}

fn seconds(time: SystemTime) -> f64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs_f64())
        .unwrap_or_default()
}

/// The files of one add-on's interpreter.
#[derive(Debug)]
pub(crate) struct Files {
    /// Longest mount point first.
    mounts: Vec<Mount>,
    /// Bytes kept in the writable folders, counted when first needed.
    used: Mutex<Option<u64>>,
}

impl Files {
    /// `addons` are the add-on folders by Kodi id (the plugin first); `data` is the add-on's
    /// own writable folder (`None` keeps its files in memory).
    pub fn new(addons: &[(String, PathBuf)], data: Option<&Path>) -> Self {
        let mut mounts = vec![
            Mount {
                at: RAYGUN_ROOT.to_owned(),
                store: Store::Raygun,
            },
            Mount {
                at: STDLIB_ROOT.to_owned(),
                store: Store::Stdlib,
            },
        ];
        for (index, (id, folder)) in addons.iter().enumerate() {
            let store = match data {
                Some(data) if index == 0 => Store::Overlay {
                    lower: folder.clone(),
                    upper: data.join("addon"),
                },
                _ => Store::Disk {
                    root: folder.clone(),
                    writable: false,
                },
            };
            mounts.push(Mount {
                at: format!("{ADDONS_ROOT}/{id}"),
                store,
            });
        }
        let memory = Arc::new(Mutex::new(MemoryTree::default()));
        for (at, name) in [(USERDATA_ROOT, "userdata"), (TEMP_ROOT, "temp")] {
            let store = match data {
                Some(data) => Store::Disk {
                    root: data.join(name),
                    writable: true,
                },
                None => Store::Memory(memory.clone()),
            };
            mounts.push(Mount {
                at: at.to_owned(),
                store,
            });
        }
        mounts.sort_by_key(|mount| std::cmp::Reverse(mount.at.len()));
        Self {
            mounts,
            used: Mutex::new(None),
        }
    }

    /// The mount a normalized path is in, and the path within it (`""` for the mount itself).
    fn locate<'a>(&'a self, path: &str) -> Option<(&'a Mount, String)> {
        self.mounts.iter().find_map(|mount| {
            if path == mount.at {
                Some((mount, String::new()))
            } else {
                path.strip_prefix(&mount.at)
                    .and_then(|rest| rest.strip_prefix('/'))
                    .map(|rest| (mount, rest.to_owned()))
            }
        })
    }

    /// Folders above the mounts (`/kodi`, `/kodi/home/addons`...), with the names under them.
    fn virtual_children(&self, path: &str) -> Option<BTreeSet<String>> {
        let prefix = if path == "/" {
            "/".to_owned()
        } else {
            format!("{path}/")
        };
        let mut children = BTreeSet::new();
        for mount in &self.mounts {
            if let Some(rest) = mount.at.strip_prefix(&prefix)
                && let Some(first) = rest.split('/').next()
            {
                children.insert(first.to_owned());
            }
        }
        (!children.is_empty()).then_some(children)
    }

    fn checked(path: &str) -> io::Result<String> {
        normalize(path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid path: '{path}'"),
            )
        })
    }

    pub fn stat(&self, path: &str) -> io::Result<Stat> {
        let path = Self::checked(path)?;
        let Some((mount, rest)) = self.locate(&path) else {
            return if self.virtual_children(&path).is_some() {
                Ok(Stat {
                    dir: true,
                    size: 0,
                    modified: 0.0,
                })
            } else {
                Err(not_found(&path))
            };
        };
        let viewed = Self::view(mount, &rest);
        let mount = &viewed;
        match &mount.store {
            Store::Overlay { .. } => unreachable!("viewed as a folder"),
            Store::Disk { root, .. } => {
                let real = root.join(&rest);
                match std::fs::metadata(&real) {
                    Ok(metadata) => Ok(Stat {
                        dir: metadata.is_dir(),
                        size: metadata.len(),
                        modified: metadata.modified().map(seconds).unwrap_or_default(),
                    }),
                    // A writable folder that doesn't exist yet reads as an empty one.
                    Err(_) if rest.is_empty() => Ok(Stat {
                        dir: true,
                        size: 0,
                        modified: 0.0,
                    }),
                    Err(_) => Err(not_found(&path)),
                }
            }
            Store::Raygun => embedded_stat(
                RAYGUN_FILES
                    .iter()
                    .map(|(name, text)| (*name, text.len() as u64)),
                &rest,
            )
            .ok_or_else(|| not_found(&path)),
            Store::Stdlib => embedded_stat(
                stdlib()
                    .iter()
                    .map(|(name, bytes)| (name.as_str(), bytes.len() as u64)),
                &rest,
            )
            .ok_or_else(|| not_found(&path)),
            Store::Memory(tree) => {
                let tree = tree
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let key = format!("{}/{rest}", mount.at);
                if rest.is_empty() || tree.dirs.contains(&key) {
                    Ok(Stat {
                        dir: true,
                        size: 0,
                        modified: 0.0,
                    })
                } else if let Some((bytes, time)) = tree.files.get(&key) {
                    Ok(Stat {
                        dir: false,
                        size: bytes.len() as u64,
                        modified: seconds(*time),
                    })
                } else {
                    Err(not_found(&path))
                }
            }
        }
    }

    pub fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let path = Self::checked(path)?;
        let (mount, rest) = self.locate(&path).ok_or_else(|| not_found(&path))?;
        let viewed = Self::view(mount, &rest);
        let mount = &viewed;
        match &mount.store {
            Store::Overlay { .. } => unreachable!("viewed as a folder"),
            Store::Disk { root, .. } => {
                let real = root.join(&rest);
                if real.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::IsADirectory,
                        format!("Is a directory: '{path}'"),
                    ));
                }
                let file = std::fs::File::open(&real).map_err(|_| not_found(&path))?;
                let mut bytes = Vec::new();
                file.take(MAX_FILE_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > MAX_FILE_BYTES {
                    return Err(io::Error::other("the file is too large"));
                }
                Ok(bytes)
            }
            Store::Raygun => RAYGUN_FILES
                .iter()
                .find(|(name, _)| *name == rest)
                .map(|(_, text)| text.as_bytes().to_vec())
                .ok_or_else(|| not_found(&path)),
            Store::Stdlib => stdlib().get(&rest).cloned().ok_or_else(|| not_found(&path)),
            Store::Memory(tree) => tree
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .files
                .get(&format!("{}/{rest}", mount.at))
                .map(|(bytes, _)| bytes.clone())
                .ok_or_else(|| not_found(&path)),
        }
    }

    /// How a mount reads at `rest`: an overlay as its upper folder where the path is there,
    /// else as its read-only lower one.
    fn view(mount: &Mount, rest: &str) -> Mount {
        match &mount.store {
            Store::Overlay { lower, upper } => {
                let root = if !rest.is_empty() && upper.join(rest).exists() {
                    upper
                } else {
                    lower
                };
                Mount {
                    at: mount.at.clone(),
                    store: Store::Disk {
                        root: root.clone(),
                        writable: false,
                    },
                }
            }
            _ => mount.clone(),
        }
    }

    /// The writable store a path is in (with the path within it), or why not. In an overlay
    /// that's its upper folder, with the folders above the path made there when they're in the
    /// lower one.
    fn writable(&self, path: &str) -> io::Result<(String, Mount, String)> {
        let path = Self::checked(path)?;
        let (mount, rest) = self.locate(&path).ok_or_else(|| read_only(&path))?;
        if rest.is_empty() {
            return Err(read_only(&path));
        }
        match &mount.store {
            Store::Disk { writable: true, .. } | Store::Memory(_) => {
                Ok((path.clone(), mount.clone(), rest))
            }
            Store::Overlay { lower, upper } => {
                if let Some((folder, _)) = rest.rsplit_once('/')
                    && !upper.join(folder).is_dir()
                    && lower.join(folder).is_dir()
                {
                    std::fs::create_dir_all(upper.join(folder))?;
                }
                Ok((
                    path.clone(),
                    Mount {
                        at: mount.at.clone(),
                        store: Store::Disk {
                            root: upper.clone(),
                            writable: true,
                        },
                    },
                    rest,
                ))
            }
            _ => Err(read_only(&path)),
        }
    }

    pub fn is_writable(&self, path: &str) -> bool {
        self.writable(path).is_ok()
    }

    /// Bytes in the writable folders now.
    fn usage(&self) -> u64 {
        let mut used = self
            .used
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(bytes) = *used {
            return bytes;
        }
        let mut total = 0;
        for mount in &self.mounts {
            match &mount.store {
                Store::Disk {
                    root,
                    writable: true,
                }
                | Store::Overlay { upper: root, .. } => total += folder_size(root),
                _ => {}
            }
        }
        *used = Some(total);
        total
    }

    fn account(&self, grown: i64) {
        let mut used = self
            .used
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(bytes) = used.as_mut() {
            *bytes = bytes.saturating_add_signed(grown);
        }
    }

    pub fn write(&self, path: &str, data: &[u8], append: bool) -> io::Result<()> {
        let (path, mount, rest) = self.writable(path)?;
        if data.len() > MAX_FILE_BYTES {
            return Err(io::Error::other("the file is too large"));
        }
        let before = self.stat(&path).map(|stat| stat.size).unwrap_or_default();
        let after = if append {
            before + data.len() as u64
        } else {
            data.len() as u64
        };
        if after > before && self.usage() + (after - before) > QUOTA_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "No space left on device: the add-on's folder is full",
            ));
        }
        match &mount.store {
            Store::Disk { root, .. } => {
                let real = root.join(&rest);
                let parent_ok = real.parent().is_some_and(Path::is_dir)
                    || real.parent() == Some(root.as_path());
                if !parent_ok {
                    return Err(not_found(&path));
                }
                if real.parent() == Some(root.as_path()) {
                    std::fs::create_dir_all(root)?;
                }
                if append {
                    use std::io::Write;
                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&real)?;
                    file.write_all(data)?;
                } else {
                    std::fs::write(&real, data)?;
                }
            }
            Store::Memory(tree) => {
                let mut tree = tree
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let key = format!("{}/{rest}", mount.at);
                let parent = key.rsplit_once('/').map(|(parent, _)| parent.to_owned());
                if parent
                    .as_ref()
                    .is_some_and(|parent| *parent != mount.at && !tree.dirs.contains(parent))
                {
                    return Err(not_found(&path));
                }
                let entry = tree
                    .files
                    .entry(key)
                    .or_insert_with(|| (Vec::new(), SystemTime::now()));
                if !append {
                    entry.0.clear();
                }
                entry.0.extend_from_slice(data);
                entry.1 = SystemTime::now();
            }
            _ => return Err(read_only(&path)),
        }
        self.account(after as i64 - before as i64);
        Ok(())
    }

    pub fn list(&self, path: &str) -> io::Result<Vec<String>> {
        let path = Self::checked(path)?;
        let Some((mount, rest)) = self.locate(&path) else {
            return self
                .virtual_children(&path)
                .map(|children| children.into_iter().collect())
                .ok_or_else(|| not_found(&path));
        };
        let mut names: BTreeSet<String> = match &mount.store {
            Store::Overlay { lower, upper } => {
                let mut names = BTreeSet::new();
                let mut found = false;
                for root in [lower, upper] {
                    if let Ok(entries) = std::fs::read_dir(root.join(&rest)) {
                        found = true;
                        names.extend(
                            entries
                                .filter_map(Result::ok)
                                .map(|entry| entry.file_name().to_string_lossy().into_owned()),
                        );
                    }
                }
                if !found && !rest.is_empty() {
                    return Err(not_found(&path));
                }
                names
            }
            Store::Disk { root, .. } => {
                let real = root.join(&rest);
                match std::fs::read_dir(&real) {
                    Ok(entries) => entries
                        .filter_map(Result::ok)
                        .map(|entry| entry.file_name().to_string_lossy().into_owned())
                        .collect(),
                    Err(_) if rest.is_empty() => BTreeSet::new(),
                    Err(_) => return Err(not_found(&path)),
                }
            }
            Store::Raygun => embedded_list(RAYGUN_FILES.iter().map(|(name, _)| *name), &rest),
            Store::Stdlib => embedded_list(stdlib().keys().map(String::as_str), &rest),
            Store::Memory(tree) => {
                let tree = tree
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let prefix = if rest.is_empty() {
                    format!("{}/", mount.at)
                } else {
                    format!("{}/{rest}/", mount.at)
                };
                tree.files
                    .keys()
                    .chain(tree.dirs.iter())
                    .filter_map(|key| key.strip_prefix(&prefix))
                    .filter(|name| !name.contains('/'))
                    .map(str::to_owned)
                    .collect()
            }
        };
        if let Some(children) = self.virtual_children(&path) {
            names.extend(children);
        }
        Ok(names.into_iter().collect())
    }

    pub fn mkdir(&self, path: &str) -> io::Result<()> {
        let (path, mount, rest) = self.writable(path)?;
        if self.stat(&path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("File exists: '{path}'"),
            ));
        }
        match &mount.store {
            Store::Disk { root, .. } => {
                let real = root.join(&rest);
                if real.parent() == Some(root.as_path()) {
                    std::fs::create_dir_all(root)?;
                }
                std::fs::create_dir(&real).map_err(|error| match error.kind() {
                    io::ErrorKind::NotFound => not_found(&path),
                    _ => error,
                })
            }
            Store::Memory(tree) => {
                let mut tree = tree
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let key = format!("{}/{rest}", mount.at);
                if let Some((parent, _)) = key.rsplit_once('/')
                    && parent != mount.at
                    && !tree.dirs.contains(parent)
                {
                    return Err(not_found(&path));
                }
                tree.dirs.insert(key);
                Ok(())
            }
            _ => Err(read_only(&path)),
        }
    }

    pub fn remove(&self, path: &str) -> io::Result<()> {
        let (path, mount, rest) = self.writable(path)?;
        let size = self.stat(&path)?;
        if size.dir {
            return Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("Is a directory: '{path}'"),
            ));
        }
        match &mount.store {
            Store::Disk { root, .. } => std::fs::remove_file(root.join(&rest))?,
            Store::Memory(tree) => {
                tree.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .files
                    .remove(&format!("{}/{rest}", mount.at));
            }
            _ => return Err(read_only(&path)),
        }
        self.account(-(size.size as i64));
        Ok(())
    }

    pub fn rmdir(&self, path: &str) -> io::Result<()> {
        let (path, mount, rest) = self.writable(path)?;
        if !self.list(&path)?.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::DirectoryNotEmpty,
                format!("Directory not empty: '{path}'"),
            ));
        }
        match &mount.store {
            Store::Disk { root, .. } => std::fs::remove_dir(root.join(&rest)),
            Store::Memory(tree) => {
                tree.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .dirs
                    .remove(&format!("{}/{rest}", mount.at));
                Ok(())
            }
            _ => Err(read_only(&path)),
        }
    }

    pub fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let (from, from_mount, from_rest) = self.writable(from)?;
        let (to, to_mount, to_rest) = self.writable(to)?;
        match (&from_mount.store, &to_mount.store) {
            (Store::Disk { root: a, .. }, Store::Disk { root: b, .. }) => {
                std::fs::rename(a.join(&from_rest), b.join(&to_rest)).map_err(|error| {
                    match error.kind() {
                        io::ErrorKind::NotFound => not_found(&from),
                        _ => error,
                    }
                })
            }
            (Store::Memory(tree), Store::Memory(_)) => {
                let mut tree = tree
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let source = format!("{}/{from_rest}", from_mount.at);
                let target = format!("{}/{to_rest}", to_mount.at);
                let found = tree.files.remove(&source).ok_or_else(|| not_found(&from))?;
                tree.files.insert(target, found);
                Ok(())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::CrossesDevices,
                format!("Invalid cross-device link: '{to}'"),
            )),
        }
    }

    /// A database file's real path, for SQLite (`None` for files kept in memory).
    pub fn database_path(&self, path: &str) -> io::Result<Option<PathBuf>> {
        let (path, mount, rest) = self.writable(path)?;
        match &mount.store {
            Store::Disk { root, .. } => {
                let real = root.join(&rest);
                if real.parent() == Some(root.as_path()) {
                    std::fs::create_dir_all(root)?;
                } else if !real.parent().is_some_and(Path::is_dir) {
                    return Err(not_found(&path));
                }
                Ok(Some(real))
            }
            _ => Ok(None),
        }
    }

    /// Looks for module `name` (its last part) in `folders`, as Python's path finder does: a
    /// package or a module in the first folder that has one, else the folders that make a
    /// namespace package.
    pub fn find_in(&self, folders: &[String], name: &str) -> Option<Found> {
        if !safe_part(name) || name.is_empty() {
            return None;
        }
        let mut portions = Vec::new();
        for folder in folders {
            let base = format!("{}/{name}", folder.trim_end_matches('/'));
            let init = format!("{base}/__init__.py");
            if self.stat(&init).is_ok_and(|stat| !stat.dir) {
                return Some(Found::Package(init, base));
            }
            let module = format!("{base}.py");
            if self.stat(&module).is_ok_and(|stat| !stat.dir) {
                return Some(Found::Module(module));
            }
            if self.stat(&base).is_ok_and(|stat| stat.dir) {
                portions.push(base);
            }
        }
        (!portions.is_empty()).then_some(Found::Namespace(portions))
    }
}

fn embedded_stat<'a>(files: impl Iterator<Item = (&'a str, u64)>, rest: &str) -> Option<Stat> {
    if rest.is_empty() {
        return Some(Stat {
            dir: true,
            size: 0,
            modified: 0.0,
        });
    }
    let folder = format!("{rest}/");
    for (name, size) in files {
        if name == rest {
            return Some(Stat {
                dir: false,
                size,
                modified: 0.0,
            });
        }
        if name.starts_with(&folder) {
            return Some(Stat {
                dir: true,
                size: 0,
                modified: 0.0,
            });
        }
    }
    None
}

fn embedded_list<'a>(files: impl Iterator<Item = &'a str>, rest: &str) -> BTreeSet<String> {
    let prefix = if rest.is_empty() {
        String::new()
    } else {
        format!("{rest}/")
    };
    files
        .filter_map(|name| name.strip_prefix(&prefix))
        .filter_map(|name| name.split('/').next())
        .map(str::to_owned)
        .collect()
}

fn folder_size(root: &Path) -> u64 {
    let mut total = 0;
    let mut pending = vec![root.to_path_buf()];
    while let Some(folder) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            match entry.metadata() {
                Ok(metadata) if metadata.is_dir() => pending.push(entry.path()),
                Ok(metadata) => total += metadata.len(),
                Err(_) => {}
            }
        }
    }
    total
}

/// Python source as text: UTF-8 (a byte-order mark dropped), else the encoding its coding line
/// names (PEP 263), with Windows line ends made plain.
pub(crate) fn source_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => {
            let head: String = String::from_utf8_lossy(&bytes[..bytes.len().min(400)])
                .lines()
                .take(2)
                .collect::<Vec<_>>()
                .join("\n");
            let label = head
                .find("coding")
                .map(|at| &head[at + 6..])
                .and_then(|rest| rest.strip_prefix([':', '=']))
                .map(|rest| {
                    rest.trim_start()
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                        .collect::<String>()
                })
                .unwrap_or_default();
            let encoding = encoding_rs::Encoding::for_label(label.as_bytes())
                .unwrap_or(encoding_rs::WINDOWS_1252);
            encoding.decode(bytes).0.into_owned()
        }
    };
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_inside() {
        assert_eq!(
            normalize("/kodi/home/../home/./addons").as_deref(),
            Some("/kodi/home/addons")
        );
        assert_eq!(
            normalize("/../../etc/passwd").as_deref(),
            Some("/etc/passwd")
        );
        assert_eq!(normalize("relative"), None);
        assert_eq!(
            normalize("/kodi/temp/a:b"),
            None,
            "no drive letters or streams"
        );
        assert_eq!(normalize("/kodi/temp/CON.txt"), None, "no Windows devices");
        assert_eq!(normalize("/kodi/temp/a\\..\\b"), None);
    }

    #[test]
    fn writable_folders_and_read_only_ones() {
        let dir = tempfile::tempdir().unwrap();
        let addon = dir.path().join("addon");
        std::fs::create_dir_all(addon.join("resources")).unwrap();
        std::fs::write(addon.join("default.py"), "print(1)").unwrap();
        let library = dir.path().join("library");
        std::fs::create_dir_all(&library).unwrap();
        let files = Files::new(
            &[
                ("plugin.video.test".to_owned(), addon.clone()),
                ("script.module.test".to_owned(), library),
            ],
            Some(&dir.path().join("data")),
        );
        assert_eq!(
            files
                .read("/kodi/home/addons/plugin.video.test/default.py")
                .unwrap(),
            b"print(1)"
        );
        // What the plugin writes in its own folder is kept in its data; its files stay.
        let own = "/kodi/home/addons/plugin.video.test";
        files
            .write(&format!("{own}/resources/cache.json"), b"[1]", false)
            .unwrap();
        files
            .write(&format!("{own}/default.py"), b"x", false)
            .unwrap();
        assert_eq!(files.read(&format!("{own}/default.py")).unwrap(), b"x");
        assert_eq!(
            std::fs::read(addon.join("default.py")).unwrap(),
            b"print(1)"
        );
        assert!(dir.path().join("data/addon/resources/cache.json").is_file());
        assert_eq!(files.list(own).unwrap(), ["default.py", "resources"]);
        assert_eq!(
            files.list(&format!("{own}/resources")).unwrap(),
            ["cache.json"]
        );
        assert!(
            files
                .write("/kodi/home/addons/script.module.test/x.py", b"x", false)
                .is_err(),
            "libraries stay read-only"
        );
        assert!(
            files
                .read("/kodi/home/addons/plugin.video.test/../../../../secret")
                .is_err()
        );
        let profile = "/kodi/home/userdata/addon_data/plugin.video.test";
        assert_eq!(
            files
                .write(&format!("{profile}/cache.json"), b"{}", false)
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound,
            "the folder has to be made first"
        );
        files.mkdir("/kodi/home/userdata/addon_data").unwrap();
        files.mkdir(profile).unwrap();
        files
            .write(&format!("{profile}/cache.json"), b"{}", false)
            .unwrap();
        files
            .write(&format!("{profile}/cache.json"), b"[]", true)
            .unwrap();
        assert_eq!(
            files.read(&format!("{profile}/cache.json")).unwrap(),
            b"{}[]"
        );
        assert_eq!(files.list(profile).unwrap(), ["cache.json"]);
        assert!(
            dir.path()
                .join("data/userdata/addon_data/plugin.video.test/cache.json")
                .is_file()
        );
        assert_eq!(
            files.list("/kodi/home/addons").unwrap(),
            ["plugin.video.test", "script.module.test"],
            "folders above the mounts list them"
        );
        assert!(files.stat("/kodi/home").unwrap().dir);
        assert!(files.stat("/etc").is_err());
        assert!(files.read(&format!("{RAYGUN_ROOT}/xbmc.py")).is_ok());
        assert!(
            files
                .write(&format!("{RAYGUN_ROOT}/xbmc.py"), b"", false)
                .is_err()
        );
    }

    #[test]
    fn modules_are_found_as_python_finds_them() {
        let files = Files::new(&[], None);
        let raygun = [RAYGUN_ROOT.to_owned()];
        assert_eq!(
            files.find_in(&raygun, "xbmc"),
            Some(Found::Module(format!("{RAYGUN_ROOT}/xbmc.py")))
        );
        assert_eq!(
            files.find_in(&raygun, "requests"),
            Some(Found::Package(
                format!("{RAYGUN_ROOT}/requests/__init__.py"),
                format!("{RAYGUN_ROOT}/requests")
            ))
        );
        let stdlib = [STDLIB_ROOT.to_owned()];
        assert!(matches!(
            files.find_in(&stdlib, "json"),
            Some(Found::Package(..))
        ));
        assert!(matches!(
            files.find_in(&stdlib, "base64"),
            Some(Found::Module(..))
        ));
        assert_eq!(
            files.find_in(&stdlib, "tkinter"),
            None,
            "left out of the build"
        );
    }

    #[test]
    fn sources_decode_as_python_does() {
        assert_eq!(source_text(b"\xEF\xBB\xBFx = 1\r\n"), "x = 1\n");
        assert_eq!(
            source_text(b"# -*- coding: latin-1 -*-\nname = '\xe9'\n"),
            "# -*- coding: latin-1 -*-\nname = '\u{e9}'\n"
        );
    }
}
