//! Regenerates `packages/bindings/src/index.ts`. Run via `pnpm bindings`.

fn main() {
    let path = raygun_app::workspace_bindings_path();
    if let Err(message) = raygun_app::export_bindings(&path) {
        eprintln!("{message}");
        std::process::exit(1);
    }
    println!("wrote {}", path.display());
}
