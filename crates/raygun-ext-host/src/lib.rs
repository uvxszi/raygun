//! Raygun's extension host (ARCHITECTURE §7): manifests, network permissions, the provider
//! registry and the QuickJS (`js`) runtime with the host API.

mod host;
mod html;
mod js;
pub mod manifest;
pub mod permissions;
mod storage;

#[cfg(test)]
mod tests;

pub use host::{
    CallError, ExtensionFiles, ExtensionHost, ExtensionSource, HostConfig, MUSIC_METADATA,
    discover_installed,
};
pub use js::Limits;
pub use manifest::{MANIFEST_FILE, Manifest, ManifestError};
