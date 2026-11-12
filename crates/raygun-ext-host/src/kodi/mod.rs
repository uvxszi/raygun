//! Kodi add-ons in Raygun (`docs/extensions/formats/kodi.md`).
//!
//! - [`index`] reads Kodi repositories (`addons.xml`) and lists their plugins;
//! - [`package`] installs a plugin with the `script.module.*` libraries it imports, from its own
//!   repository or Kodi's;
//! - [`addon`] reads an add-on's `addon.xml`, settings and strings;
//! - `python` runs its Python in RustPython with Raygun's `xbmc*` modules (`lib/`): files kept to
//!   the add-on's own folders, the network through Raygun's client, no processes or sockets;
//! - [`runtime`] answers Raygun's calls by running the plugin (`plugin://` addresses), and
//!   `catalog` maps what it lists onto Raygun's video and music sources.

pub mod addon;
mod catalog;
pub mod index;
pub mod package;
mod python;
pub mod runtime;

#[cfg(test)]
mod tests;

pub use index::{UNSUPPORTED, index_address, parse, refusal, unpack};
pub use runtime::KodiExtension;

/// The kind a video plugin provides (music plugins provide `music.metadata` and `music.stream`).
pub(crate) const VIDEO_SOURCE: &str = crate::video::VIDEO_SOURCE;

/// Kodi's own repository (Omega): the libraries plugins need come from here when their own
/// repository doesn't have them, as Kodi installs them.
pub const OFFICIAL_REPOSITORY: &str = "https://mirrors.kodi.tv/addons/omega/addons.xml.gz";
