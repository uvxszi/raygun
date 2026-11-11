//! Reading aloud with voices downloaded to this device: Piper's and Kokoro's models, run by
//! sherpa-onnx (its C API, downloaded on demand as the catalogue's runtime and loaded at run
//! time). The app splits text into [`sentences`], picks a voice for the book's language
//! ([`language::guess`], [`select::pick`]), reads Roman numerals in titles as numbers
//! ([`numerals`]) and synthesizes sentence by sentence ahead of what plays.

pub mod language;
pub mod numerals;
pub mod pcm;
pub mod select;
pub mod sentences;
mod sherpa;

pub use sherpa::{Engine, Runtime, Speech, Synthesizer, TtsError};

/// The runtime that runs voice models.
pub const RUNTIME_ENGINE: &str = "sherpa-onnx";
