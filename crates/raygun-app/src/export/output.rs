//! Files written outside the app's own storage (M46.1).
//!
//! On Android the `raygun-audio` plugin makes each file (in `Download/Raygun/`,
//! `Pictures/Raygun/` or a folder the user picked) and hands over an open descriptor, since Rust
//! can't open `content://` URIs; the file is written here like any other and published when
//! done. Elsewhere the same places are plain folders under `<data root>/exports/`. Files for the
//! share sheet go to the cache's `share/` folder.

use std::fs::File;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};

use tauri::AppHandle;

/// Where a file goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// `Download/Raygun/`.
    Downloads,
    /// `Pictures/Raygun/`.
    Pictures,
    /// A folder the user picked: its tree URI and name.
    Folder { uri: String, name: String },
    /// The cache, for the share sheet or the clipboard.
    Share,
}

impl Place {
    /// How the user sees the place's top folder.
    fn shown(&self) -> String {
        match self {
            Self::Downloads => "Download/Raygun".into(),
            Self::Pictures => "Pictures/Raygun".into(),
            Self::Folder { name, .. } => name.clone(),
            Self::Share => String::new(),
        }
    }
}

/// A file being written. Finish it with [`Outputs::finish`], which publishes or removes it.
pub struct Output {
    pub file: File,
    /// Where it is, as the user sees it ("Download/Raygun/Manga/Frieren/0001 Chapter 1.cbz").
    pub shown: String,
    target: Target,
}

enum Target {
    /// A file Android made for us (MediaStore or a picked folder).
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    Native(String),
    /// A plain file, written beside its final name and moved there when done.
    Path { partial: PathBuf, path: PathBuf },
}

/// A finished file.
pub struct Saved {
    pub shown: String,
    /// Set for files in the share folder, to hand to the share sheet.
    pub path: Option<PathBuf>,
}

#[derive(Clone)]
pub struct Outputs {
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    app: AppHandle,
    /// Where saved files go off Android (shared storage is MediaStore's there).
    #[cfg_attr(target_os = "android", allow(dead_code))]
    exports: PathBuf,
    share: PathBuf,
}

impl Outputs {
    pub fn new(app: AppHandle, exports: PathBuf, share: PathBuf) -> Self {
        Self {
            app,
            exports,
            share,
        }
    }

    /// Makes (or replaces) `name` under the folders `path` in `place`. Blocking.
    pub fn create(
        &self,
        place: &Place,
        path: &[String],
        name: &str,
        mime: &str,
    ) -> Result<Output, String> {
        let shown = std::iter::once(place.shown())
            .chain(path.iter().cloned())
            .chain(std::iter::once(name.to_owned()))
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        if *place == Place::Share {
            return plain(self.share.join(name), shown);
        }
        self.create_native(place, path, name, mime, shown)
    }

    #[cfg(target_os = "android")]
    fn create_native(
        &self,
        place: &Place,
        path: &[String],
        name: &str,
        mime: &str,
        shown: String,
    ) -> Result<Output, String> {
        use std::os::fd::FromRawFd;

        use tauri::Manager;
        use tauri_plugin_raygun_audio::{NativeAudio, OutputRequest};

        let (kind, tree) = match place {
            Place::Downloads => ("downloads", None),
            Place::Pictures => ("pictures", None),
            Place::Folder { uri, .. } => ("tree", Some(uri.as_str())),
            Place::Share => unreachable!("share files are plain"),
        };
        let made = self
            .app
            .state::<NativeAudio<tauri::Wry>>()
            .create_output(&OutputRequest {
                place: kind,
                tree,
                path,
                name,
                mime,
            })?;
        // SAFETY: Kotlin detached this descriptor for us (`detachFd`); nothing else owns it.
        let file = unsafe { File::from_raw_fd(made.fd) };
        Ok(Output {
            file,
            shown,
            target: Target::Native(made.uri),
        })
    }

    #[cfg(not(target_os = "android"))]
    fn create_native(
        &self,
        place: &Place,
        path: &[String],
        name: &str,
        _mime: &str,
        shown: String,
    ) -> Result<Output, String> {
        let top = match place {
            Place::Downloads => "Download",
            Place::Pictures => "Pictures",
            Place::Folder { .. } => "Folder",
            Place::Share => unreachable!("share files are plain"),
        };
        let mut dir = self.exports.join(top);
        for part in path {
            dir.push(part);
        }
        plain(dir.join(name), shown)
    }

    /// Publishes a written file (`ok`), or removes it. Blocking.
    pub fn finish(&self, output: Output, ok: bool) -> Result<Saved, String> {
        let Output {
            mut file,
            shown,
            target,
        } = output;
        let flushed = file.flush().and_then(|()| file.sync_all());
        drop(file);
        let ok = ok && flushed.is_ok();
        match target {
            Target::Native(_uri) => {
                #[cfg(target_os = "android")]
                {
                    use tauri::Manager;
                    use tauri_plugin_raygun_audio::NativeAudio;
                    self.app
                        .state::<NativeAudio<tauri::Wry>>()
                        .finish_output(&_uri, ok)?;
                }
                flushed.map_err(|error| error.to_string())?;
                Ok(Saved { shown, path: None })
            }
            Target::Path { partial, path } => {
                if !ok {
                    let _ = std::fs::remove_file(&partial);
                    flushed.map_err(|error| error.to_string())?;
                    return Err("The file wasn't finished".into());
                }
                std::fs::rename(&partial, &path).map_err(|error| error.to_string())?;
                let shared = path.starts_with(&self.share).then_some(path);
                Ok(Saved {
                    shown,
                    path: shared,
                })
            }
        }
    }

    /// Clears what earlier shares left in the cache.
    pub fn clear_shared(&self) {
        let _ = std::fs::remove_dir_all(&self.share);
    }

    /// Opens the share sheet for a file in the share folder.
    pub fn share(&self, path: &Path, mime: &str, title: &str) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            use tauri::Manager;
            use tauri_plugin_raygun_audio::NativeAudio;
            self.app.state::<NativeAudio<tauri::Wry>>().share_file(
                &path.to_string_lossy(),
                mime,
                title,
            )
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (path, mime, title);
            Err("Sharing works on Android only for now".into())
        }
    }

    /// Puts an image from the share folder on the clipboard.
    pub fn copy_image(&self, path: &Path, label: &str) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            use tauri::Manager;
            use tauri_plugin_raygun_audio::NativeAudio;
            self.app
                .state::<NativeAudio<tauri::Wry>>()
                .copy_image(&path.to_string_lossy(), label)
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (path, label);
            Err("Copying images works on Android only for now".into())
        }
    }
}

fn plain(path: PathBuf, shown: String) -> Result<Output, String> {
    let dir = path.parent().ok_or("The file has no folder")?;
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let mut partial = path.clone().into_os_string();
    partial.push(".part");
    let partial = PathBuf::from(partial);
    let file = File::create(&partial).map_err(|error| error.to_string())?;
    Ok(Output {
        file,
        shown,
        target: Target::Path { partial, path },
    })
}

/// Whether `file` can seek (a pipe from some document providers can't): ZIP files are then
/// written in streaming form.
pub fn seekable(file: &mut File) -> bool {
    file.stream_position().is_ok()
}
