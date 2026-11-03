//! Whisper's tokenizer, for decoding only: the byte-level BPE vocabulary and special tokens read
//! from `tokenizer.json`, so the `tokenizers` crate isn't needed.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::SpeechError;

pub struct Tokens {
    /// Each id's text in GPT-2's byte-to-character form.
    pieces: HashMap<u32, String>,
    /// Special tokens by name (`<|endoftext|>`, `<|en|>`, …).
    special: HashMap<String, u32>,
    special_ids: HashSet<u32>,
    bytes: HashMap<char, u8>,
}

impl Tokens {
    pub fn from_json(text: &str) -> Result<Self, SpeechError> {
        let json: Value =
            serde_json::from_str(text).map_err(|error| SpeechError::Files(error.to_string()))?;
        let vocab = json
            .pointer("/model/vocab")
            .and_then(Value::as_object)
            .ok_or_else(|| SpeechError::Files("tokenizer.json has no vocabulary".into()))?;
        let mut pieces: HashMap<u32, String> = vocab
            .iter()
            .filter_map(|(piece, id)| Some((u32::try_from(id.as_u64()?).ok()?, piece.clone())))
            .collect();
        let mut special = HashMap::new();
        let mut special_ids = HashSet::new();
        for token in json
            .get("added_tokens")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (Some(id), Some(content)) = (
                token
                    .get("id")
                    .and_then(Value::as_u64)
                    .and_then(|id| u32::try_from(id).ok()),
                token.get("content").and_then(Value::as_str),
            ) else {
                continue;
            };
            if content.starts_with("<|") && content.ends_with("|>") {
                special.insert(content.to_owned(), id);
                special_ids.insert(id);
                pieces.remove(&id);
            } else {
                pieces.insert(id, content.to_owned());
            }
        }
        Ok(Self {
            pieces,
            special,
            special_ids,
            bytes: byte_table(),
        })
    }

    /// A special token's id.
    pub fn id(&self, name: &str) -> Option<u32> {
        self.special.get(name).copied()
    }

    /// The language tokens (`<|en|>`…): the special tokens between start-of-transcript and
    /// translate, with their codes.
    pub fn languages(&self) -> Vec<(u32, String)> {
        let (Some(start), Some(end)) = (self.id("<|startoftranscript|>"), self.id("<|translate|>"))
        else {
            return Vec::new();
        };
        let mut languages: Vec<(u32, String)> = self
            .special
            .iter()
            .filter(|(_, id)| **id > start && **id < end)
            .map(|(name, id)| (*id, name[2..name.len() - 2].to_owned()))
            .collect();
        languages.sort();
        languages
    }

    /// The text of ordinary tokens; special ones are left out.
    pub fn decode(&self, ids: &[u32]) -> String {
        let mut bytes = Vec::new();
        for id in ids {
            if self.special_ids.contains(id) {
                continue;
            }
            let Some(piece) = self.pieces.get(id) else {
                continue;
            };
            for character in piece.chars() {
                match self.bytes.get(&character) {
                    Some(byte) => bytes.push(*byte),
                    None => {
                        let mut buffer = [0u8; 4];
                        bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
                    }
                }
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// GPT-2's `bytes_to_unicode`, reversed: printable bytes stand for themselves, the rest were
/// moved to code points from 256 up.
fn byte_table() -> HashMap<char, u8> {
    let printable = |byte: u8| matches!(byte, b'!'..=b'~' | 0xA1..=0xAC | 0xAE..=0xFF);
    let mut table = HashMap::with_capacity(256);
    let mut moved = 0u32;
    for byte in 0..=255u8 {
        let character = if printable(byte) {
            char::from(byte)
        } else {
            moved += 1;
            char::from_u32(255 + moved).unwrap_or('\u{FFFD}')
        };
        table.insert(character, byte);
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "added_tokens": [
        {"id": 5, "content": "<|endoftext|>", "special": true},
        {"id": 6, "content": "<|startoftranscript|>", "special": true},
        {"id": 7, "content": "<|en|>", "special": true},
        {"id": 8, "content": "<|fr|>", "special": true},
        {"id": 9, "content": "<|translate|>", "special": true}
      ],
      "model": {"type": "BPE", "vocab": {"Hello": 0, "Ġworld": 1, "!": 2, "Ã©": 3, "t": 4}}
    }"#;

    #[test]
    fn decodes_byte_level_pieces() {
        let tokens = Tokens::from_json(SAMPLE).unwrap();
        assert_eq!(tokens.decode(&[6, 0, 1, 2, 5]), "Hello world!");
        // "é" is two bytes, each written as its own character.
        assert_eq!(tokens.decode(&[3, 4]), "ét");
    }

    #[test]
    fn finds_special_and_language_tokens() {
        let tokens = Tokens::from_json(SAMPLE).unwrap();
        assert_eq!(tokens.id("<|endoftext|>"), Some(5));
        assert_eq!(
            tokens.languages(),
            vec![(7, "en".to_owned()), (8, "fr".to_owned())]
        );
    }
}
