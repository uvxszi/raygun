// Tauri links against comctl32 v6 on Windows. The app crate gets the required
// manifest from tauri-build; this crate's own test and tool binaries need it too,
// or they die at startup with STATUS_ENTRYPOINT_NOT_FOUND. Link args from a
// dependency's build script don't propagate, so the app crate is unaffected.
fn main() {
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
