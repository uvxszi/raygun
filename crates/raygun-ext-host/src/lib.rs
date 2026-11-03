//! Raygun's extension host (ARCHITECTURE §7): manifests, network permissions, the provider
//! registry and the QuickJS (`js`) runtime with the host API.

pub mod audiobook;
mod content;
pub mod cookies;
mod host;
mod html;
mod js;
pub mod manga;
pub mod manifest;
pub mod novel;
pub mod nuclear;
pub mod package;
pub mod permissions;
pub mod podcast;
pub mod repo;
pub mod shosetsu;
mod storage;
pub mod video;

#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod nuclear_tests;
#[cfg(test)]
mod repo_tests;
#[cfg(test)]
mod tests;

pub use content::ContentFilter;
pub use host::{
    CallError, ExtensionFiles, ExtensionHost, ExtensionSource, HostConfig, LYRICS, MUSIC_CHARTS,
    MUSIC_METADATA, MUSIC_STREAM, MUSIC_TAGS, ProviderCandidates, ResolvedStream, UserSettings,
    discover_installed,
};
pub use js::Limits;
pub use manifest::{MANIFEST_FILE, Manifest, ManifestError};
