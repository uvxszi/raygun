//! Regenerates `packages/bindings/src/index.ts` and the settings' defaults beside it. Run via
//! `pnpm bindings`.

fn main() {
    let path = raygun_app::workspace_bindings_path();
    if let Err(message) = raygun_app::export_bindings(&path) {
        eprintln!("{message}");
        std::process::exit(1);
    }
    println!("wrote {}", path.display());
    let defaults = path.with_file_name("settings-defaults.json");
    if let Err(error) = std::fs::write(&defaults, raygun_app::settings_defaults_json()) {
        eprintln!("failed to write {}: {error}", defaults.display());
        std::process::exit(1);
    }
    println!("wrote {}", defaults.display());
}
