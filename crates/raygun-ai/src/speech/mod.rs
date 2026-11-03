//! Speech to text (M46.32): Whisper on this device (candle, CPU) and any OpenAI-compatible
//! `audio/transcriptions` endpoint. Both take 16 kHz mono samples a window at a time and answer
//! timed segments, relative to the window's start.

mod mel;
mod online;
mod tokens;
mod whisper;

pub use mel::mel_filters;
pub use online::{transcribe_online, wav_bytes};
pub use tokens::Tokens;
pub use whisper::{Heard, Whisper};

/// Whisper's sample rate; callers resample to it.
pub const SAMPLE_RATE: u32 = 16_000;
/// The longest window one call takes (Whisper's 30 seconds).
pub const WINDOW_MS: u32 = 30_000;

/// A stretch of speech, in milliseconds from the window's start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SpeechError {
    #[error("the speech model's files couldn't be read: {0}")]
    Files(String),
    #[error("the speech model failed: {0}")]
    Model(String),
    #[error("couldn't reach the transcriber: {0}")]
    Network(String),
    #[error("the transcriber answered {status}: {message}")]
    Provider { status: u16, message: String },
}

impl From<candle_core::Error> for SpeechError {
    fn from(error: candle_core::Error) -> Self {
        Self::Model(error.to_string())
    }
}
