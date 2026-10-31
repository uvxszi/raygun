use raygun_model::{AppInfo, StorageInfo, StorageMode};

use crate::startup::AppPaths;

/// Reports build and runtime facts about the running app.
#[tauri::command]
#[specta::specta]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Raygun".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        debug: cfg!(debug_assertions),
    }
}

/// Reports where Raygun keeps its data on this device.
#[tauri::command]
#[specta::specta]
pub fn storage_info(paths: tauri::State<'_, AppPaths>) -> StorageInfo {
    to_storage_info(&paths.0)
}

fn to_storage_info(paths: &raygun_paths::Paths) -> StorageInfo {
    StorageInfo {
        mode: match paths.mode() {
            raygun_paths::StorageMode::Portable => StorageMode::Portable,
            raygun_paths::StorageMode::Installed => StorageMode::Installed,
            raygun_paths::StorageMode::Mobile => StorageMode::Mobile,
            raygun_paths::StorageMode::Override => StorageMode::Override,
        },
        root: paths.root().display().to_string(),
        logs: paths.logs().display().to_string(),
        webview: paths.webview().display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn app_info_reports_host_platform() {
        let info = app_info();
        assert_eq!(info.name, "Raygun");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.platform, std::env::consts::OS);
        assert_eq!(info.arch, std::env::consts::ARCH);
    }

    #[test]
    fn storage_info_mirrors_resolved_paths() {
        let root = PathBuf::from("portable-root");
        let paths = raygun_paths::Paths::new(root.clone(), raygun_paths::StorageMode::Portable);

        let info = to_storage_info(&paths);

        assert_eq!(info.mode, StorageMode::Portable);
        assert_eq!(info.root, root.display().to_string());
        assert_eq!(info.logs, root.join("logs").display().to_string());
        assert_eq!(info.webview, root.join("webview").display().to_string());
    }
}
