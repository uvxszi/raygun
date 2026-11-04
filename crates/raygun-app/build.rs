use std::path::PathBuf;

fn main() {
    bundle_release_notes();
    link_common_controls();
}

/// Every `docs/releases/v<version>.md`, bundled so "What's new" works offline: writes
/// `$OUT_DIR/release_notes.rs`, a list of `(version, notes)`.
fn bundle_release_notes() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let dir = manifest.join("../../docs/releases");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut entries: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .map(|read| {
            read.filter_map(Result::ok)
                .filter_map(|entry| {
                    let path = entry.path();
                    let name = path.file_name()?.to_str()?;
                    let version = name.strip_suffix(".md")?.strip_prefix('v')?.to_owned();
                    println!("cargo:rerun-if-changed={}", path.display());
                    Some((version, path))
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    let mut out = String::from("&[\n");
    for (version, path) in entries {
        out.push_str(&format!(
            "    ({version:?}, include_str!({:?})),\n",
            path.to_string_lossy()
        ));
    }
    out.push(']');
    let target = PathBuf::from(std::env::var("OUT_DIR").expect("out dir")).join("release_notes.rs");
    std::fs::write(target, out).expect("release notes list written");
}

// Tauri links against comctl32 v6 on Windows. The app crate gets the required
// manifest from tauri-build; this crate's own test and tool binaries need it too,
// or they die at startup with STATUS_ENTRYPOINT_NOT_FOUND. Link args from a
// dependency's build script don't propagate, so the app crate is unaffected.
fn link_common_controls() {
    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !windows_msvc {
        return;
    }
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );
}
