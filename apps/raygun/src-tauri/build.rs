fn main() {
    // The exe's own icon is embedded here (a Windows resource), so a new icon set has to re-run
    // this script; tauri-build doesn't watch the files, and the old icon stayed in the exe (M46.7).
    println!("cargo:rerun-if-changed=icons");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    tauri_build::build()
}
