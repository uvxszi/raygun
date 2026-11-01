//! Raygun's extension host (ARCHITECTURE §7): manifests, network permissions, the provider
//! registry and the QuickJS (`js`) runtime with the host API.

mod host;
mod html;
mod js;
pub mod manga;
pub mod manifest;
pub mod nuclear;
pub mod package;
pub mod permissions;
pub mod repo;
mod storage;

#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod nuclear_tests;
#[cfg(test)]
mod repo_tests;
#[cfg(test)]
mod tests;

pub use host::{
    CallError, ExtensionFiles, ExtensionHost, ExtensionSource, HostConfig, LYRICS, MUSIC_METADATA,
    MUSIC_STREAM, ProviderCandidates, ResolvedStream, UserSettings, discover_installed,
};
pub use js::Limits;
pub use manifest::{MANIFEST_FILE, Manifest, ManifestError};
