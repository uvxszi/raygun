//! Reading aloud with voices downloaded to this device: Piper's and Kokoro's models, run by
//! sherpa-onnx (its C API, downloaded on demand as the catalogue's runtime and loaded at run
//! time). The app splits text into [`sentences`] (and long ones into pieces made one at a time,
//! [`sentences::piece_end`]), picks a voice for the book's language ([`language::guess`],
//! [`select::pick`]), reads Roman numerals in titles as numbers ([`numerals`]) and abbreviations
//! in full ([`abbreviations`]), and synthesizes ahead of what plays on the device's fast cores
//! ([`threads`]).
//!
//! The catalogue's voices are full precision: sherpa-onnx's int8 builds of them run slower (2–6×
//! on a desktop, about 2× on phones by published figures), as ONNX Runtime does much of their
//! quantized convolutions on one thread.

pub mod abbreviations;
pub mod language;
pub mod numerals;
pub mod pcm;
pub mod select;
pub mod sentences;
mod sherpa;
mod threads;

pub use sherpa::{Engine, Runtime, Speech, Synthesizer, TtsError};
pub use threads::threads;

/// The runtime that runs voice models.
pub const RUNTIME_ENGINE: &str = "sherpa-onnx";
