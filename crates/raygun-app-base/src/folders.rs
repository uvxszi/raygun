//! Folders picked with the system's picker (Android) or a folder dialog (desktop), and access to
//! the device's media.

#[cfg(target_os = "android")]
pub async fn pick_folder(
    app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .pick_folder()
    })
    .await
    .map_err(|error| error.to_string())?
}

/// A folder for backups (M46.51): the system picker on Android, a folder dialog on desktop.
#[cfg(target_os = "android")]
pub async fn pick_backup_folder(
    app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    pick_folder(app).await
}

#[cfg(not(target_os = "android"))]
pub async fn pick_backup_folder(
    _app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Where Raygun saves backups")
        .pick_folder()
        .await;
    Ok(match picked {
        Some(folder) => {
            let path = folder.path().to_path_buf();
            tauri_plugin_raygun_audio::PickedFolder {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .or_else(|| Some(path.to_string_lossy().into_owned())),
                uri: Some(path.to_string_lossy().into_owned()),
            }
        }
        None => tauri_plugin_raygun_audio::PickedFolder {
            uri: None,
            name: None,
        },
    })
}

/// A folder for Local Files (M46.27): the system picker on Android, a folder dialog on desktop.
#[cfg(target_os = "android")]
pub async fn pick_local_folder(
    app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    pick_folder(app).await
}

#[cfg(not(target_os = "android"))]
pub async fn pick_local_folder(
    _app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Add a folder to Local Files")
        .pick_folder()
        .await;
    Ok(match picked {
        Some(folder) => {
            let path = folder.path().to_path_buf();
            tauri_plugin_raygun_audio::PickedFolder {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .or_else(|| Some(path.to_string_lossy().into_owned())),
                uri: Some(path.to_string_lossy().into_owned()),
            }
        }
        None => tauri_plugin_raygun_audio::PickedFolder {
            uri: None,
            name: None,
        },
    })
}

/// Asks to read the device's songs and videos (Android); desktop needs nothing.
#[cfg(target_os = "android")]
pub async fn request_media_access(app: &tauri::AppHandle) -> bool {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .request_media_access()
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

#[cfg(not(target_os = "android"))]
pub async fn request_media_access(_app: &tauri::AppHandle) -> bool {
    true
}

/// The system's folder dialog on desktop (M46.6); a picked folder is a plain path there.
#[cfg(not(target_os = "android"))]
pub async fn pick_folder(
    _app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Where Raygun saves downloads")
        .pick_folder()
        .await;
    Ok(match picked {
        Some(folder) => {
            let path = folder.path().to_path_buf();
            tauri_plugin_raygun_audio::PickedFolder {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .or_else(|| Some(path.to_string_lossy().into_owned())),
                uri: Some(path.to_string_lossy().into_owned()),
            }
        }
        None => tauri_plugin_raygun_audio::PickedFolder {
            uri: None,
            name: None,
        },
    })
}
