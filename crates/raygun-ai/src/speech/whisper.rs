//! Whisper on the CPU with candle: one 30-second window per call, greedy decoding with
//! timestamps and OpenAI's timestamp rules (pairs, never backwards, timestamp-first), the
//! language detected from the first window unless given. Greedy decoding loops on music and
//! noise ("la la la…" to the token limit): a repeating tail is cut, and the rest of the window
//! is skipped.

use std::path::Path;

use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as m, Config, audio, model};

use super::{Segment, SpeechError, Tokens, mel_filters};

/// Milliseconds per timestamp token.
const TIMESTAMP_MS: u32 = 20;
/// The first timestamp may be at most this far in (OpenAI's `max_initial_timestamp`, 1 s).
const MAX_INITIAL_TIMESTAMP: u32 = 50;
/// A run of text tokens repeated back to back is a decoding loop when it spans at least this
/// many tokens (and repeats at least [`LOOP_REPEATS`] times)…
const LOOP_SPAN: usize = 24;
const LOOP_REPEATS: usize = 3;
/// …with a repeating unit of at most this many tokens.
const LOOP_UNIT: usize = 16;

pub struct Whisper {
    model: model::Whisper,
    tokens: Tokens,
    filters: Vec<f32>,
    device: Device,
    sot: u32,
    eot: u32,
    transcribe: u32,
    timestamp_begin: u32,
    no_speech: Option<u32>,
    /// Added to every step's logits: −∞ for tokens never written.
    suppress: Vec<f32>,
    languages: Vec<(u32, String)>,
}

/// One window's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heard {
    pub segments: Vec<Segment>,
    /// How much of the window was used: the next window starts here, so a sentence cut at the
    /// window's end is heard again whole.
    pub consumed_ms: u32,
    pub language: Option<String>,
}

impl Whisper {
    /// Loads `config.json`, `tokenizer.json` and `model.safetensors` from a model folder. The
    /// weights are memory-mapped rather than read into a buffer first, which halves what a load
    /// takes at its peak (a phone's memory killer could end the app mid-load).
    pub fn load(dir: &Path) -> Result<Self, SpeechError> {
        let read = |name: &str| {
            std::fs::read(dir.join(name))
                .map_err(|error| SpeechError::Files(format!("{name}: {error}")))
        };
        let config: Config = serde_json::from_slice(&read("config.json")?)
            .map_err(|error| SpeechError::Files(format!("config.json: {error}")))?;
        let tokens = Tokens::from_json(&String::from_utf8_lossy(&read("tokenizer.json")?))?;
        let device = Device::Cpu;
        let weights_path = dir.join("model.safetensors");
        if !weights_path.is_file() {
            return Err(SpeechError::Files("model.safetensors is missing".into()));
        }
        // SAFETY: the file is the catalogue's download, pinned by its SHA-256 and only replaced
        // by a new download into a fresh folder; nothing writes to it while it's mapped.
        let weights =
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], m::DTYPE, &device)? };
        let model = model::Whisper::load(&weights, config.clone())?;
        let special = |name: &str| {
            tokens
                .id(name)
                .ok_or_else(|| SpeechError::Files(format!("tokenizer.json has no {name}")))
        };
        let sot = special(m::SOT_TOKEN)?;
        let eot = special(m::EOT_TOKEN)?;
        let transcribe = special(m::TRANSCRIBE_TOKEN)?;
        let no_timestamps = special(m::NO_TIMESTAMPS_TOKEN)?;
        let no_speech = m::NO_SPEECH_TOKENS.iter().find_map(|name| tokens.id(name));
        // Timestamps follow `<|notimestamps|>`.
        let timestamp_begin = no_timestamps + 1;
        let suppress = (0..config.vocab_size as u32)
            .map(|id| {
                let special = id > eot && id < timestamp_begin;
                if special || config.suppress_tokens.contains(&id) {
                    f32::NEG_INFINITY
                } else {
                    0.0
                }
            })
            .collect();
        let filters = mel_filters(config.num_mel_bins, m::N_FFT, m::SAMPLE_RATE as f64);
        let languages = tokens.languages();
        Ok(Self {
            model,
            tokens,
            filters,
            device,
            sot,
            eot,
            transcribe,
            timestamp_begin,
            no_speech,
            suppress,
            languages,
        })
    }

    /// Transcribes up to 30 s of 16 kHz mono samples. `language` is a code such as `en`; `None`
    /// detects it.
    pub fn transcribe(
        &mut self,
        samples: &[f32],
        language: Option<&str>,
    ) -> Result<Heard, SpeechError> {
        self.transcribe_until(samples, language, &|| false)
    }

    /// [`Self::transcribe`], giving up with [`SpeechError::Cancelled`] once `cancelled` says so
    /// (checked between tokens: a window takes seconds on a phone).
    pub fn transcribe_until(
        &mut self,
        samples: &[f32],
        language: Option<&str>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Heard, SpeechError> {
        if cancelled() {
            return Err(SpeechError::Cancelled);
        }
        let content = samples.len().min(m::N_SAMPLES);
        let content_ms = (content as u64 * 1000 / m::SAMPLE_RATE as u64) as u32;
        let mut padded = samples[..content].to_vec();
        padded.resize(m::N_SAMPLES, 0.0);
        let n_mels = self.model.config.num_mel_bins;
        let limit = self.model.config.max_target_positions / 2;
        let mel = audio::pcm_to_mel(&self.model.config, &padded, &self.filters);
        let frames = mel.len() / n_mels;
        let mel = Tensor::from_vec(mel, (1, n_mels, frames), &self.device)?.narrow(
            2,
            0,
            frames.min(m::N_FRAMES),
        )?;
        let features = self.model.encoder.forward(&mel, true)?;

        // The start-of-transcript position answers both the language and "is anyone talking".
        let first = self.logits(&[self.sot], &features, true)?;
        let no_speech_prob = self
            .no_speech
            .map(|id| softmax_at(&first, id as usize))
            .unwrap_or(0.0);
        let language_token = language
            .and_then(|code| self.tokens.id(&format!("<|{code}|>")))
            .or_else(|| {
                self.languages
                    .iter()
                    .max_by(|a, b| first[a.0 as usize].total_cmp(&first[b.0 as usize]))
                    .map(|(id, _)| *id)
            });
        let language = language_token.and_then(|token| {
            self.languages
                .iter()
                .find(|(id, _)| *id == token)
                .map(|(_, code)| code.clone())
        });

        let mut prompt = vec![self.sot];
        prompt.extend(language_token);
        prompt.push(self.transcribe);
        let mut sampled: Vec<u32> = Vec::new();
        let mut sum_logprob = 0.0f64;
        let mut ended = false;
        let mut looped = false;
        for step in 0..limit {
            if cancelled() {
                return Err(SpeechError::Cancelled);
            }
            let mut sequence = prompt.clone();
            sequence.extend(&sampled);
            let mut logits = self.logits(&sequence, &features, step == 0)?;
            for (logit, suppress) in logits.iter_mut().zip(&self.suppress) {
                *logit += suppress;
            }
            self.apply_timestamp_rules(&mut logits, &sampled);
            let (next, _) = logits
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map(|(index, logit)| (index as u32, *logit))
                .unwrap_or((self.eot, 0.0));
            sum_logprob += log_softmax_at(&logits, next as usize);
            if next == self.eot {
                ended = true;
                break;
            }
            sampled.push(next);
            if let Some(cut) = self.loop_start(&sampled) {
                sampled.truncate(cut);
                ended = true;
                looped = true;
                break;
            }
        }
        let avg_logprob = sum_logprob / (sampled.len() + 1) as f64;
        if no_speech_prob > m::NO_SPEECH_THRESHOLD && avg_logprob < m::LOGPROB_THRESHOLD {
            return Ok(Heard {
                segments: Vec::new(),
                consumed_ms: content_ms,
                language,
            });
        }
        let (segments, consumed_ms) = self.segments(&sampled, ended, content_ms);
        // A loop would be heard again in the same place: go past it.
        let consumed_ms = if looped { content_ms } else { consumed_ms };
        Ok(Heard {
            segments,
            consumed_ms,
            language,
        })
    }

    /// Where a decoding loop's second round starts in `sampled`, when its text tokens end in one
    /// unit repeated back to back (timestamps between rounds don't count).
    fn loop_start(&self, sampled: &[u32]) -> Option<usize> {
        let text: Vec<usize> = (0..sampled.len())
            .filter(|index| sampled[*index] < self.timestamp_begin)
            .collect();
        let tokens: Vec<u32> = text.iter().map(|index| sampled[*index]).collect();
        let cut = loop_start(&tokens)?;
        Some(text[cut])
    }

    /// The next token's logits after `sequence`.
    fn logits(
        &mut self,
        sequence: &[u32],
        features: &Tensor,
        flush: bool,
    ) -> Result<Vec<f32>, SpeechError> {
        let input = Tensor::new(sequence, &self.device)?.unsqueeze(0)?;
        let hidden = self.model.decoder.forward(&input, features, flush)?;
        let last = hidden.dim(1)? - 1;
        let logits = self
            .model
            .decoder
            .final_linear(&hidden.i((..1, last..))?)?
            .i(0)?
            .i(0)?
            .to_dtype(DType::F32)?;
        Ok(logits.to_vec1::<f32>()?)
    }

    /// OpenAI's `ApplyTimestampRules`.
    fn apply_timestamp_rules(&self, logits: &mut [f32], sampled: &[u32]) {
        let begin = self.timestamp_begin as usize;
        if begin >= logits.len() {
            return;
        }
        let is_time = |token: &u32| *token >= self.timestamp_begin;
        let last_was_time = sampled.last().is_some_and(is_time);
        let penultimate_was_time = sampled.len() < 2 || is_time(&sampled[sampled.len() - 2]);
        if last_was_time {
            if penultimate_was_time {
                logits[begin..].fill(f32::NEG_INFINITY);
            } else {
                logits[..self.eot as usize].fill(f32::NEG_INFINITY);
            }
        }
        if let Some(last) = sampled.iter().rev().find(|token| is_time(token)) {
            let floor = if last_was_time && !penultimate_was_time {
                *last
            } else {
                last + 1
            } as usize;
            let end = floor.min(logits.len());
            logits[begin..end].fill(f32::NEG_INFINITY);
        }
        if sampled.is_empty() {
            logits[..begin].fill(f32::NEG_INFINITY);
            let last_allowed = begin + MAX_INITIAL_TIMESTAMP as usize;
            if last_allowed + 1 < logits.len() {
                logits[last_allowed + 1..].fill(f32::NEG_INFINITY);
            }
        }
        // When time is likelier than any one word, write a timestamp.
        let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if max.is_finite() {
            let time_mass: f64 = logits[begin..]
                .iter()
                .map(|logit| ((logit - max) as f64).exp())
                .sum();
            let best_text = logits[..begin]
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            if time_mass.ln() > (best_text - max) as f64 {
                logits[..begin].fill(f32::NEG_INFINITY);
            }
        }
    }

    /// Turns sampled tokens into segments. A segment cut off by the window's end is left for
    /// the next window, which starts where it began.
    fn segments(&self, sampled: &[u32], ended: bool, content_ms: u32) -> (Vec<Segment>, u32) {
        let mut segments = Vec::new();
        let mut start: Option<u32> = None;
        let mut text: Vec<u32> = Vec::new();
        for token in sampled {
            if *token >= self.timestamp_begin {
                let time = ((token - self.timestamp_begin) * TIMESTAMP_MS).min(content_ms);
                if text.is_empty() {
                    start = Some(time);
                } else {
                    let words = self.tokens.decode(&text);
                    let words = words.trim();
                    if !words.is_empty() {
                        segments.push(Segment {
                            start_ms: start.unwrap_or(0),
                            end_ms: time.max(start.unwrap_or(0)),
                            text: words.to_owned(),
                        });
                    }
                    text.clear();
                    start = Some(time);
                }
            } else {
                text.push(*token);
            }
        }
        let single_ending = sampled.len() >= 2
            && sampled[sampled.len() - 1] >= self.timestamp_begin
            && sampled[sampled.len() - 2] < self.timestamp_begin;
        let mut consumed = content_ms;
        if !text.is_empty() || (!single_ending && !sampled.is_empty()) {
            // Unfinished: hear it again from where it started, if that moves us forward.
            match start {
                Some(from) if from >= 1000 && from < content_ms => consumed = from,
                _ if !text.is_empty() && ended => {
                    let words = self.tokens.decode(&text);
                    if !words.trim().is_empty() {
                        segments.push(Segment {
                            start_ms: start.unwrap_or(0),
                            end_ms: content_ms,
                            text: words.trim().to_owned(),
                        });
                    }
                }
                _ => {}
            }
        }
        // Greedy decoding sometimes loops; drop a line said again right after itself.
        segments.dedup_by(|later, earlier| later.text == earlier.text);
        (segments, consumed)
    }
}

/// Where the second round of a loop at the end of `tokens` starts: the shortest unit repeated
/// back to back at least [`LOOP_REPEATS`] times over at least [`LOOP_SPAN`] tokens.
fn loop_start(tokens: &[u32]) -> Option<usize> {
    for unit in 1..=LOOP_UNIT {
        let repeats = LOOP_SPAN.div_ceil(unit).max(LOOP_REPEATS);
        let span = unit * repeats;
        if tokens.len() < span {
            continue;
        }
        let tail = &tokens[tokens.len() - span..];
        let first = &tail[..unit];
        if tail.chunks(unit).all(|chunk| chunk == first) {
            // Back to where the run began.
            let mut start = tokens.len() - span;
            while start >= unit && tokens[start - unit..start] == tokens[start..start + unit] {
                start -= unit;
            }
            return Some(start + unit);
        }
    }
    None
}

fn softmax_at(logits: &[f32], index: usize) -> f64 {
    log_softmax_at(logits, index).exp()
}

fn log_softmax_at(logits: &[f32], index: usize) -> f64 {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max) as f64;
    let sum: f64 = logits.iter().map(|logit| (*logit as f64 - max).exp()).sum();
    logits
        .get(index)
        .map_or(f64::NEG_INFINITY, |logit| *logit as f64 - max - sum.ln())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_loops_are_cut_after_their_first_round() {
        // "la" said 30 times: one is kept.
        let mut tokens = vec![7, 8, 9];
        tokens.extend(std::iter::repeat_n(5, 30));
        assert_eq!(loop_start(&tokens), Some(4));
        // A three-token phrase eight times.
        let mut phrase = vec![1, 2];
        for _ in 0..8 {
            phrase.extend([10, 11, 12]);
        }
        assert_eq!(loop_start(&phrase), Some(5));
        // A chorus sung twice isn't a loop.
        let mut chorus: Vec<u32> = (100..112).collect();
        chorus.extend(100..112);
        assert_eq!(loop_start(&chorus), None);
        assert_eq!(loop_start(&[1, 2, 3, 3, 3]), None);
    }
}
