//! Translation on this device (M46.48): OPUS-MT's Marian models on the CPU with candle, the way
//! candle's `marian-mt` example runs them. Text becomes SentencePiece pieces (`spm`), mapped to
//! ids through the model's `vocab.json` as Hugging Face's `MarianTokenizer` does; decoding is
//! greedy with the KV cache, and the padding token is never written. One line at a time: the
//! encoder takes no padding mask, so batching lines would let them disturb each other.

mod plan;
mod spm;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::marian::{self, MTModel};
use serde::Deserialize;

pub use plan::{
    Plan, Step, base_and_script, has_other_letters, looks_english, plan, same_language,
};

#[derive(Debug, thiserror::Error)]
pub enum MarianError {
    #[error("the translation model's files couldn't be read: {0}")]
    Files(String),
    #[error("the translation model failed: {0}")]
    Model(String),
    #[error("this model can't write {0}")]
    NoTarget(String),
}

impl From<candle_core::Error> for MarianError {
    fn from(error: candle_core::Error) -> Self {
        Self::Model(error.to_string())
    }
}

/// The files a Marian model folder holds.
pub const FILES: [&str; 4] = [
    "config.json",
    "vocab.json",
    "source.spm",
    "model.safetensors",
];

fn yes() -> bool {
    true
}

/// What candle needs of a Marian `config.json`, with the defaults older configs leave out.
#[derive(Debug, Deserialize)]
struct RawConfig {
    vocab_size: usize,
    #[serde(default)]
    decoder_vocab_size: Option<usize>,
    max_position_embeddings: usize,
    encoder_layers: usize,
    encoder_ffn_dim: usize,
    encoder_attention_heads: usize,
    decoder_layers: usize,
    decoder_ffn_dim: usize,
    decoder_attention_heads: usize,
    #[serde(default = "yes")]
    use_cache: bool,
    #[serde(default = "yes")]
    is_encoder_decoder: bool,
    activation_function: candle_nn::Activation,
    d_model: usize,
    decoder_start_token_id: u32,
    #[serde(default = "yes")]
    scale_embedding: bool,
    pad_token_id: u32,
    eos_token_id: u32,
    #[serde(default)]
    forced_eos_token_id: Option<u32>,
    #[serde(default = "yes")]
    share_encoder_decoder_embeddings: bool,
    /// Tokens never written (the padding token).
    #[serde(default)]
    bad_words_ids: Vec<Vec<u32>>,
}

impl RawConfig {
    fn to_candle(&self) -> marian::Config {
        marian::Config {
            vocab_size: self.vocab_size,
            decoder_vocab_size: Some(self.decoder_vocab_size.unwrap_or(self.vocab_size)),
            max_position_embeddings: self.max_position_embeddings,
            encoder_layers: self.encoder_layers,
            encoder_ffn_dim: self.encoder_ffn_dim,
            encoder_attention_heads: self.encoder_attention_heads,
            decoder_layers: self.decoder_layers,
            decoder_ffn_dim: self.decoder_ffn_dim,
            decoder_attention_heads: self.decoder_attention_heads,
            use_cache: self.use_cache,
            is_encoder_decoder: self.is_encoder_decoder,
            activation_function: self.activation_function,
            d_model: self.d_model,
            decoder_start_token_id: self.decoder_start_token_id,
            scale_embedding: self.scale_embedding,
            pad_token_id: self.pad_token_id,
            eos_token_id: self.eos_token_id,
            forced_eos_token_id: self.forced_eos_token_id.unwrap_or(self.eos_token_id),
            share_encoder_decoder_embeddings: self.share_encoder_decoder_embeddings,
        }
    }
}

pub struct Marian {
    model: MTModel,
    config: marian::Config,
    source: spm::SentencePiece,
    vocab: HashMap<String, u32>,
    /// Each id's piece.
    pieces: Vec<String>,
    unknown: u32,
    /// Never written: the padding token.
    banned: Vec<u32>,
    /// Left out of the text: end, padding, unknown and language tokens.
    special: HashSet<u32>,
    device: Device,
}

impl Marian {
    /// Loads `config.json`, `vocab.json`, `source.spm` and `model.safetensors` from a folder.
    pub fn load(dir: &Path) -> Result<Self, MarianError> {
        let read = |name: &str| {
            std::fs::read(dir.join(name))
                .map_err(|error| MarianError::Files(format!("{name}: {error}")))
        };
        let raw: RawConfig = serde_json::from_slice(&read("config.json")?)
            .map_err(|error| MarianError::Files(format!("config.json: {error}")))?;
        let config = raw.to_candle();
        let vocab: HashMap<String, u32> = serde_json::from_slice(&read("vocab.json")?)
            .map_err(|error| MarianError::Files(format!("vocab.json: {error}")))?;
        let source = spm::SentencePiece::from_bytes(&read("source.spm")?)?;
        let size = config.decoder_vocab_size.unwrap_or(config.vocab_size);
        let mut pieces = vec![String::new(); size];
        for (piece, id) in &vocab {
            if let Some(slot) = pieces.get_mut(*id as usize) {
                slot.clone_from(piece);
            }
        }
        let unknown = vocab.get("<unk>").copied().unwrap_or(1);
        let mut special: HashSet<u32> = [config.eos_token_id, config.pad_token_id, unknown]
            .into_iter()
            .collect();
        special.extend(
            vocab
                .iter()
                .filter(|(piece, _)| piece.starts_with(">>") && piece.ends_with("<<"))
                .map(|(_, id)| *id),
        );
        let mut banned: Vec<u32> = raw.bad_words_ids.iter().flatten().copied().collect();
        if !banned.contains(&config.pad_token_id) {
            banned.push(config.pad_token_id);
        }
        let device = Device::Cpu;
        let weights_path = dir.join("model.safetensors");
        // SAFETY: the model manager writes the folder once, before it counts as installed, and
        // nothing changes the file while it's mapped; the weights are copied out as they load.
        let weights =
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DType::F32, &device)? };
        let model = MTModel::new(&config, weights)?;
        Ok(Self {
            model,
            config,
            source,
            vocab,
            pieces,
            unknown,
            banned,
            special,
            device,
        })
    }

    /// Whether the model knows the `>>code<<` token that picks an output language.
    pub fn knows(&self, token: &str) -> bool {
        self.vocab.contains_key(token)
    }

    /// Translates one line; `target_token` (`>>deu<<`) picks the language of a model that writes
    /// several. Blank text stays blank.
    pub fn translate(
        &mut self,
        text: &str,
        target_token: Option<&str>,
    ) -> Result<String, MarianError> {
        let pieces = self.source.pieces(text)?;
        if pieces.is_empty() {
            return Ok(String::new());
        }
        let mut ids: Vec<u32> = Vec::with_capacity(pieces.len() + 2);
        if let Some(token) = target_token {
            let id = self
                .vocab
                .get(token)
                .copied()
                .ok_or_else(|| MarianError::NoTarget(token.to_owned()))?;
            ids.push(id);
        }
        ids.extend(
            pieces
                .iter()
                .map(|piece| self.vocab.get(piece).copied().unwrap_or(self.unknown)),
        );
        // Room for the end token within the model's positions.
        ids.truncate(self.config.max_position_embeddings.saturating_sub(2).max(8));
        ids.push(self.config.eos_token_id);
        let result = self.generate(&ids);
        self.model.reset_kv_cache();
        result
    }

    fn generate(&mut self, ids: &[u32]) -> Result<String, MarianError> {
        let input = Tensor::new(ids, &self.device)?.unsqueeze(0)?;
        let encoded = self.model.encoder().forward(&input, 0)?;
        let mut output = vec![self.config.decoder_start_token_id];
        // A line's translation is rarely more than twice its length; this stops a model that loops.
        let most = (ids.len() * 2 + 8).min(self.config.max_position_embeddings - 1);
        for step in 0..most {
            let context = if step == 0 { output.len() } else { 1 };
            let start = output.len() - context;
            let tokens = Tensor::new(&output[start..], &self.device)?.unsqueeze(0)?;
            let logits = self.model.decode(&tokens, &encoded, start)?.squeeze(0)?;
            let last = logits.dim(0)? - 1;
            let mut logits: Vec<f32> = logits.get(last)?.to_dtype(DType::F32)?.to_vec1()?;
            for banned in &self.banned {
                if let Some(logit) = logits.get_mut(*banned as usize) {
                    *logit = f32::NEG_INFINITY;
                }
            }
            let next = logits
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map_or(self.config.eos_token_id, |(index, _)| index as u32);
            if next == self.config.eos_token_id || next == self.config.forced_eos_token_id {
                break;
            }
            output.push(next);
            if stuck(&output) {
                break;
            }
        }
        let pieces: Vec<&str> = output[1..]
            .iter()
            .filter(|id| !self.special.contains(id))
            .filter_map(|id| self.pieces.get(*id as usize).map(String::as_str))
            .collect();
        Ok(spm::join(&pieces))
    }
}

/// The same token eight times running: greedy decoding fell into a loop.
fn stuck(output: &[u32]) -> bool {
    output.len() >= 8
        && output[output.len() - 8..]
            .windows(2)
            .all(|pair| pair[0] == pair[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_configs_get_defaults() {
        // opus-mt-en-de's config: no decoder vocabulary size, no shared embeddings flag.
        let raw: RawConfig = serde_json::from_str(
            r#"{
                "vocab_size": 58101, "max_position_embeddings": 512, "encoder_layers": 6,
                "encoder_ffn_dim": 2048, "encoder_attention_heads": 8, "decoder_layers": 6,
                "decoder_ffn_dim": 2048, "decoder_attention_heads": 8, "activation_function": "swish",
                "d_model": 512, "decoder_start_token_id": 58100, "pad_token_id": 58100,
                "eos_token_id": 0, "bad_words_ids": [[58100]]
            }"#,
        )
        .unwrap();
        let config = raw.to_candle();
        assert_eq!(config.decoder_vocab_size, Some(58101));
        assert_eq!(config.forced_eos_token_id, 0);
        assert!(config.share_encoder_decoder_embeddings && config.scale_embedding);
        assert_eq!(raw.bad_words_ids, [[58100]]);
    }

    #[test]
    fn loops_are_noticed() {
        assert!(!stuck(&[1, 2, 2, 2]));
        assert!(stuck(&[5, 3, 3, 3, 3, 3, 3, 3, 3]));
        assert!(!stuck(&[3, 3, 3, 3, 3, 3, 3, 4]));
    }

    /// Downloads the many-languages-to-English model into a temporary folder through the model
    /// manager and translates a few lines: `cargo test -p raygun-ai -- --ignored marian_live
    /// --nocapture`. `MARIAN_DIR=<model folder>` uses a model already downloaded instead.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "downloads a 310 MB model"]
    async fn marian_live() {
        let model = crate::models::find("opus-mt-mul-en").unwrap();
        let temp = tempfile::tempdir().unwrap();
        let dir = match std::env::var("MARIAN_DIR") {
            Ok(dir) => std::path::PathBuf::from(dir),
            Err(_) => {
                let client = raygun_net::http_client().unwrap();
                let started = std::time::Instant::now();
                crate::models::download(&client, temp.path(), &model, |_, _| {})
                    .await
                    .unwrap();
                println!("downloaded in {:?}", started.elapsed());
                crate::models::model_dir(temp.path(), &model)
            }
        };
        tokio::task::spawn_blocking(move || {
            let started = std::time::Instant::now();
            let mut marian = Marian::load(&dir).unwrap();
            println!("loaded in {:?}", started.elapsed());
            for (line, expected) in [
                ("Je t'aime plus que tout.", "love"),
                ("Ich habe dich so vermisst.", "miss"),
                ("君の名前を呼んでいる", "name"),
                ("", ""),
            ] {
                let started = std::time::Instant::now();
                let translated = marian.translate(line, None).unwrap();
                println!("{line} → {translated} ({:?})", started.elapsed());
                assert!(
                    translated.to_lowercase().contains(expected),
                    "{line} → {translated}"
                );
            }
        })
        .await
        .unwrap();
    }
}
