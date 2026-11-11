//! Raygun's AI subsystem (ARCHITECTURE §8): the encrypted secrets store, the model manager, the
//! translation providers, translation on this device, speech to text and text to speech. Pure
//! logic; the app wires it to settings and commands.

pub mod marian;
pub mod models;
pub mod secrets;
pub mod speech;
pub mod translate;
pub mod tts;
