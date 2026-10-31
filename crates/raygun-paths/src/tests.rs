use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;

fn env(os: &str, exe: PathBuf) -> Environment {
    Environment {
        os: os.into(),
        exe,
        raygun_home: None,
        appimage: None,
        system_dir: None,
    }
}

fn ready(resolution: Resolution) -> Paths {
    match resolution {
        Resolution::Ready(paths) => paths,
        other => panic!("expected Ready, got {other:?}"),
    }
}

/// A "folder" that is really a file, so nothing can be created inside it.
fn unwritable_base(temp: &TempDir) -> PathBuf {
    let base = temp.path().join("read-only-install");
    fs::write(&base, "not a directory").unwrap();
    base
}

#[test]
fn raygun_home_overrides_everything() {
    let temp = TempDir::new().unwrap();
    let mut environment = env("windows", temp.path().join("raygun.exe"));
    environment.raygun_home = Some(temp.path().join("custom"));

    let paths = ready(resolve(&environment).unwrap());

    assert_eq!(paths.mode(), StorageMode::Override);
    assert_eq!(paths.root(), temp.path().join("custom"));
}

#[test]
fn portable_root_sits_next_to_the_executable() {
    let temp = TempDir::new().unwrap();
    let environment = env("windows", temp.path().join("raygun.exe"));

    let paths = ready(resolve(&environment).unwrap());

    assert_eq!(paths.mode(), StorageMode::Portable);
    assert_eq!(paths.root(), temp.path().join(DATA_DIR_NAME));
    assert!(paths.root().is_dir());
    assert_eq!(
        fs::read_dir(paths.root()).unwrap().count(),
        0,
        "write probe must be cleaned up"
    );
}

#[test]
fn existing_portable_data_wins_over_installed_marker() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(DATA_DIR_NAME)).unwrap();
    let system_dir = temp.path().join("system");
    fs::create_dir_all(&system_dir).unwrap();
    fs::write(system_dir.join(INSTALLED_MARKER), "").unwrap();
    let mut environment = env("windows", temp.path().join("raygun.exe"));
    environment.system_dir = Some(system_dir);

    let paths = ready(resolve(&environment).unwrap());

    assert_eq!(paths.mode(), StorageMode::Portable);
}

#[test]
fn unwritable_folder_without_marker_needs_a_choice() {
    let temp = TempDir::new().unwrap();
    let base = unwritable_base(&temp);
    let environment = env("windows", base.join("raygun.exe"));

    let resolution = resolve(&environment).unwrap();

    assert_eq!(
        resolution,
        Resolution::NeedsChoice {
            reason: ChoiceReason::Unwritable,
            portable_root: base.join(DATA_DIR_NAME),
        }
    );
}

#[test]
fn choosing_installed_mode_is_remembered() {
    let temp = TempDir::new().unwrap();
    let base = unwritable_base(&temp);
    let mut environment = env("windows", base.join("raygun.exe"));
    environment.system_dir = Some(temp.path().join("system"));

    let chosen = choose_installed(&environment).unwrap();
    let paths = ready(resolve(&environment).unwrap());

    assert_eq!(chosen, paths);
    assert_eq!(paths.mode(), StorageMode::Installed);
    assert_eq!(paths.root(), temp.path().join("system"));
}

#[test]
fn choosing_installed_mode_requires_a_system_dir() {
    let environment = env("windows", PathBuf::from("C:/Apps/raygun.exe"));

    assert!(matches!(
        choose_installed(&environment),
        Err(ChooseError::Resolve(ResolveError::MissingSystemDir))
    ));
}

#[test]
fn mobile_uses_the_sandbox() {
    let mut environment = env("android", PathBuf::from("/data/app/lib/libraygun_lib.so"));
    environment.system_dir = Some(PathBuf::from("/data/user/0/app.raygun.player"));

    let paths = ready(resolve(&environment).unwrap());

    assert_eq!(paths.mode(), StorageMode::Mobile);
    assert_eq!(paths.root(), Path::new("/data/user/0/app.raygun.player"));
}

#[test]
fn mobile_without_sandbox_is_an_error() {
    let environment = env("ios", PathBuf::from("/var/containers/Raygun.app/Raygun"));

    assert_eq!(resolve(&environment), Err(ResolveError::MissingSystemDir));
}

#[test]
fn appimage_data_goes_next_to_the_appimage_file() {
    let temp = TempDir::new().unwrap();
    let mut environment = env(
        "linux",
        PathBuf::from("/tmp/.mount_RaygunXYZ/usr/bin/raygun"),
    );
    environment.appimage = Some(temp.path().join("Raygun.AppImage"));

    let paths = ready(resolve(&environment).unwrap());

    assert_eq!(paths.root(), temp.path().join(DATA_DIR_NAME));
}

#[test]
fn appimage_variable_is_ignored_off_linux() {
    let temp = TempDir::new().unwrap();
    let mut environment = env("windows", temp.path().join("raygun.exe"));
    environment.appimage = Some(PathBuf::from("/elsewhere/Raygun.AppImage"));

    let paths = ready(resolve(&environment).unwrap());

    assert_eq!(paths.root(), temp.path().join(DATA_DIR_NAME));
}

#[test]
fn macos_data_goes_next_to_the_app_bundle() {
    let temp = TempDir::new().unwrap();
    let exe = temp.path().join("Raygun.app/Contents/MacOS/raygun");
    let environment = env("macos", exe);

    let paths = ready(resolve(&environment).unwrap());

    assert_eq!(paths.root(), temp.path().join(DATA_DIR_NAME));
}

#[test]
fn macos_translocation_needs_a_choice() {
    let exe = PathBuf::from(
        "/private/var/folders/xy/T/AppTranslocation/1234-ABCD/d/Raygun.app/Contents/MacOS/raygun",
    );
    let environment = env("macos", exe);

    let resolution = resolve(&environment).unwrap();

    assert!(matches!(
        resolution,
        Resolution::NeedsChoice {
            reason: ChoiceReason::Translocated,
            ..
        }
    ));
}

#[test]
fn well_known_locations_live_under_the_root() {
    let paths = Paths::new(PathBuf::from("/data"), StorageMode::Portable);

    assert_eq!(paths.db(), Path::new("/data/raygun.db"));
    assert_eq!(paths.settings(), Path::new("/data/settings.toml"));
    assert_eq!(paths.webview(), Path::new("/data/webview"));
    assert_eq!(paths.logs(), Path::new("/data/logs"));
    assert_eq!(
        paths.cache(CacheKind::Enhanced),
        Path::new("/data/cache/enhanced")
    );
}

#[test]
fn ensure_base_dirs_creates_logs_and_webview() {
    let temp = TempDir::new().unwrap();
    let paths = Paths::new(temp.path().join("root"), StorageMode::Portable);

    paths.ensure_base_dirs().unwrap();

    assert!(paths.logs().is_dir());
    assert!(paths.webview().is_dir());
}
