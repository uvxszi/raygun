//! Thin Tauri shell: owns the config/context, everything else lives in `raygun-app`.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    raygun_app::builder()
        .run(tauri::generate_context!())
        .expect("error while running Raygun");
}
