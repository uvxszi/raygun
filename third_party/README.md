# Vendored third-party code

## rquickjs-sys 0.14.0
Upstream: https://github.com/DelSkayn/rquickjs (MIT). It bundles QuickJS-ng (MIT).

Vendored unchanged, except for **four added binding files** so the crate builds for Android without bindgen/libclang:

| Added file | Copied from |
|---|---|
| `src/bindings/aarch64-linux-android.rs` | `aarch64-unknown-linux-gnu.rs` |
| `src/bindings/x86_64-linux-android.rs` | `x86_64-unknown-linux-gnu.rs` |
| `src/bindings/armv7-linux-androideabi.rs` | `armv7-unknown-linux-gnueabihf.rs` |
| `src/bindings/i686-linux-android.rs` | `i686-unknown-linux-gnu.rs` |

This is safe because the generated bindings only describe QuickJS's own C API: they contain no libc types, and the upstream glibc and musl bindings for the same architecture are byte-for-byte identical. Android uses the same data model as Linux on each architecture.

Wired in through `[patch.crates-io]` in the workspace `Cargo.toml`. Drop this copy once upstream ships Android bindings.
