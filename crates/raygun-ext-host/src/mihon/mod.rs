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
//!   Jsoup-compatible document model, Kotlin's helpers and the themes (`themes/*.js`).
//!
//! An extension that can't be converted faithfully (image descrambling, custom crypto, a
//! WebView) is refused at install with the reason, which the catalogue then shows.

pub mod convert;
pub mod gradle;
pub mod index;
pub mod kotlin;
mod proto;
pub(crate) mod runtime;
pub mod source;
pub mod transpile;

#[cfg(test)]
mod live_tests;
#[cfg(test)]
mod tests;

pub use convert::{CONVERTER_VERSION, Converted, convert, package};
pub use index::{Store, StoreExtension, detect, load, to_repo_index};
pub use source::{SourceFolder, Sources, locate};
