//! Thin Tauri shell: owns the config/context, everything else lives in `raygun-app`.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    raygun_app::builder()
        .build(tauri::generate_context!())
        .expect("error while building Raygun")
        .run(raygun_app::on_event);
}
