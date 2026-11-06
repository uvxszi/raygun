//! Mihon (Tachiyomi) extensions: their stores, read as Raygun repositories, and their source code,
//! converted into Raygun extensions that run in Raygun's own runtime on every platform.
//!
//! - `index` reads a store (`index.min.json`, `repo.json`, the protobuf `index.pb`) and lists its
//!   extensions as catalogue entries of format `mihon`.
//! - `source` finds an extension's code (a folder of the store's GitHub repository) and reads its
//!   build file and Kotlin files; `gradle` reads the build file.
//! - `kotlin` parses Kotlin; `convert` turns a themed extension (Madara, MangaThemesia…) into the
//!   values its theme takes, `transpile` turns a theme-less one into JavaScript.
//! - `runtime` and `prelude.js` are the runtime `mihon`: Raygun's JavaScript host with a
//!   Jsoup-compatible document model, Kotlin's helpers and the themes (`themes/*.js`);
//!   `pictures` and `text` draw the pictures sources' interceptors put back together.
//!
//! An extension that can't be converted faithfully (a WebView, code the converter doesn't read)
//! is refused at install with the reason, which the catalogue then shows.

pub mod apk;
pub mod convert;
pub mod gradle;
pub mod index;
pub mod kotlin;
mod pictures;
mod proto;
pub(crate) mod runtime;
pub mod source;
mod text;
pub mod transpile;

#[cfg(test)]
mod live_tests;
#[cfg(test)]
mod tests;

pub use convert::{CONVERTER_VERSION, Converted, convert, package};
pub use index::{
    Store, StoreExtension, detect, extension_id, is_index, load, parse, to_repo_index,
};
pub use source::{SourceFolder, Sources, locate};

/// What installing a Mihon extension answers where the `mihon` runtime isn't present. Every
/// build has it now: extensions are converted from source code, on every platform; one that
/// can't be converted says why instead.
pub const UNSUPPORTED: &str = "Mihon extensions aren't supported on this device yet";
/// What a downloaded APK waits for: a container to run an extension's own code in, which Raygun
/// doesn't have on this device yet (`docs/extensions/formats/mihon.md`, "Running source code").
pub const CONTAINER_UNAVAILABLE: &str =
    "Saved and checked. Running an extension's own code isn't available on this device yet.";

/// Whether this build has the `mihon` runtime (it always does: conversions run everywhere).
pub const fn runtime_present() -> bool {
    true
}
