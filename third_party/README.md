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

## mp3lame-sys 0.1.11
Upstream: https://github.com/DoumanAsh/mp3lame-sys (LGPL-3.0). It bundles LAME 3.100 (LGPL-2.0).

Only the encoder's sources are kept (`lame-3.100/include`, `lame-3.100/libmp3lame`, the licences), and the build is Raygun's:

| File | Change |
|---|---|
| `build.rs` | Rewritten: `cc` builds the encoder for every target. Upstream ran autotools on Unix hosts and copied `configMS.h` on Windows hosts, so a Windows machine building for Android used MSVC's config, whose `#define int8_t signed char` breaks Android's `<stdint.h>`. |
| `lame-3.100/raygun_config.h` | Added: the one configuration header (ANSI headers, `<stdint.h>`, upstream's float typedefs, SSE on 64-bit MSVC as before). |
| `Cargo.toml` | `cc` for every host, no autotools; `publish = false`. |

`src/lib.rs` (the bindings) is unchanged. Wired in through `[patch.crates-io]` in the workspace `Cargo.toml` (M46.18, saving songs as MP3).

## librqbit-sha1-wrapper 9.1.0
Upstream: https://github.com/ikatson/rqbit (Apache-2.0, `LICENSE-APACHE`). It's the hashing shim of librqbit, the torrent engine `raygun-torrent` uses.

Upstream hashes with one of two C libraries: OpenSSL (through `crypto-hash`, which has no Android build here) or aws-lc. Raygun's copy hashes with `ring`, which the workspace already builds for rustls on every target, so torrents add no C crypto library:

| File | Change |
|---|---|
| `src/lib.rs` | SHA-1 and SHA-256 from `ring::digest`, whatever feature is chosen; a SHA-1 test added. |
| `Cargo.toml` | `ring` instead of `crypto-hash`/`aws-lc-rs`; the features `sha1-crypto-hash` and `sha1-ring` kept (empty) so librqbit's feature lists resolve; `publish = false`. |

Wired in through `[patch.crates-io]` in the workspace `Cargo.toml`.
