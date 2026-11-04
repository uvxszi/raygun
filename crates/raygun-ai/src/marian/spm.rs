//! SentencePiece for OPUS-MT's `source.spm`: the model file's pieces and normalization read with
//! a small protobuf reader, segmented by the `tokenizers` crate's Unigram model and normalized by
//! its precompiled charsmap, as Hugging Face's `MarianTokenizer` does with the sentencepiece
//! library.

use tokenizers::models::unigram::Unigram;
use tokenizers::normalizers::Precompiled;
use tokenizers::{NormalizedString, Normalizer};

use super::MarianError;

/// What `▁` stands for: a space.
pub const SPACE: char = '▁';

/// A piece's kind, as the model file stores it.
const UNKNOWN: u64 = 2;

pub struct SentencePiece {
    model: Unigram,
    normalizer: Option<Precompiled>,
    add_dummy_prefix: bool,
    remove_extra_whitespaces: bool,
}

impl SentencePiece {
    /// Reads a `.spm` model file (a `ModelProto`).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MarianError> {
        let mut pieces: Vec<(String, f64)> = Vec::new();
        let mut unknown = None;
        let mut charsmap: Option<&[u8]> = None;
        let mut add_dummy_prefix = true;
        let mut remove_extra_whitespaces = true;
        for field in Fields::new(bytes) {
            match field? {
                // repeated SentencePiece pieces = 1
                (1, Value::Bytes(piece)) => {
                    let mut text = String::new();
                    let mut score = 0f32;
                    let mut kind = 1;
                    for field in Fields::new(piece) {
                        match field? {
                            (1, Value::Bytes(value)) => {
                                text = String::from_utf8_lossy(value).into_owned();
                            }
                            (2, Value::Fixed32(value)) => score = f32::from_bits(value),
                            (3, Value::Varint(value)) => kind = value,
                            _ => {}
                        }
                    }
                    if kind == UNKNOWN && unknown.is_none() {
                        unknown = Some(pieces.len());
                    }
                    pieces.push((text, f64::from(score)));
                }
                // NormalizerSpec normalizer_spec = 3
                (3, Value::Bytes(spec)) => {
                    for field in Fields::new(spec) {
                        match field? {
                            (2, Value::Bytes(value)) => charsmap = Some(value),
                            (3, Value::Varint(value)) => add_dummy_prefix = value != 0,
                            (4, Value::Varint(value)) => remove_extra_whitespaces = value != 0,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        if pieces.is_empty() {
            return Err(MarianError::Files("the tokenizer has no pieces".into()));
        }
        let normalizer = charsmap
            .filter(|map| !map.is_empty())
            .map(Precompiled::from)
            .transpose()
            .map_err(|error| MarianError::Files(format!("the tokenizer's normalizer: {error}")))?;
        let model = Unigram::from(pieces, Some(unknown.unwrap_or(0)), false)
            .map_err(|error| MarianError::Files(format!("the tokenizer: {error}")))?;
        Ok(Self {
            model,
            normalizer,
            add_dummy_prefix,
            remove_extra_whitespaces,
        })
    }

    /// The pieces `text` splits into, each a string (`▁` for spaces).
    pub fn pieces(&self, text: &str) -> Result<Vec<String>, MarianError> {
        let mut normalized = NormalizedString::from(text);
        if let Some(normalizer) = &self.normalizer {
            normalizer
                .normalize(&mut normalized)
                .map_err(|error| MarianError::Model(error.to_string()))?;
        }
        let mut text = normalized.get().to_owned();
        if self.remove_extra_whitespaces {
            text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        }
        if text.is_empty() {
            return Ok(Vec::new());
        }
        if self.add_dummy_prefix {
            text.insert(0, ' ');
        }
        let text = text.replace(' ', &SPACE.to_string());
        self.model
            .encode(&text)
            .map_err(|error| MarianError::Model(error.to_string()))
    }
}

/// Pieces back to text: `▁` is a space, and the leading one goes.
pub fn join(pieces: &[&str]) -> String {
    let text: String = pieces.concat().replace(SPACE, " ");
    text.trim().to_owned()
}

enum Value<'a> {
    Varint(u64),
    Fixed32(u32),
    Bytes(&'a [u8]),
    Skipped,
}

/// A protobuf message's fields, one `(number, value)` at a time.
struct Fields<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Fields<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    fn varint(&mut self) -> Result<u64, MarianError> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = *self
                .data
                .get(self.at)
                .ok_or_else(|| MarianError::Files("the tokenizer file is cut short".into()))?;
            self.at += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte < 0x80 {
                return Ok(value);
            }
        }
        Err(MarianError::Files("the tokenizer file is damaged".into()))
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], MarianError> {
        let end = self
            .at
            .checked_add(len)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(|| MarianError::Files("the tokenizer file is cut short".into()))?;
        let slice = &self.data[self.at..end];
        self.at = end;
        Ok(slice)
    }
}

impl<'a> Iterator for Fields<'a> {
    type Item = Result<(u64, Value<'a>), MarianError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.at >= self.data.len() {
            return None;
        }
        let field = (|| {
            let key = self.varint()?;
            let value = match key & 7 {
                0 => Value::Varint(self.varint()?),
                1 => {
                    self.take(8)?;
                    Value::Skipped
                }
                2 => {
                    let len = usize::try_from(self.varint()?)
                        .map_err(|_| MarianError::Files("the tokenizer file is damaged".into()))?;
                    Value::Bytes(self.take(len)?)
                }
                5 => {
                    let bytes = self.take(4)?;
                    Value::Fixed32(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                }
                _ => return Err(MarianError::Files("the tokenizer file is damaged".into())),
            };
            Ok((key >> 3, value))
        })();
        if field.is_err() {
            // Stop after the first damaged field.
            self.at = self.data.len();
        }
        Some(field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn varint(mut value: u64, out: &mut Vec<u8>) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }

    fn bytes_field(number: u64, data: &[u8], out: &mut Vec<u8>) {
        varint(number << 3 | 2, out);
        varint(data.len() as u64, out);
        out.extend_from_slice(data);
    }

    fn piece(text: &str, score: f32, kind: u64) -> Vec<u8> {
        let mut out = Vec::new();
        bytes_field(1, text.as_bytes(), &mut out);
        varint(2 << 3 | 5, &mut out);
        out.extend_from_slice(&score.to_le_bytes());
        varint(3 << 3, &mut out);
        varint(kind, &mut out);
        out
    }

    /// A model file with a handful of pieces and no charsmap.
    fn model() -> Vec<u8> {
        let mut out = Vec::new();
        for (text, score, kind) in [
            ("<unk>", 0.0, 2),
            ("<s>", 0.0, 3),
            ("</s>", 0.0, 3),
            ("▁", -3.0, 1),
            ("▁hello", -2.0, 1),
            ("▁wor", -4.0, 1),
            ("ld", -4.0, 1),
            ("▁world", -3.5, 1),
            ("l", -6.0, 1),
            ("d", -6.0, 1),
            ("o", -6.0, 1),
            ("w", -6.0, 1),
            ("r", -6.0, 1),
        ] {
            bytes_field(1, &piece(text, score, kind), &mut out);
        }
        let mut spec = Vec::new();
        bytes_field(1, b"identity", &mut spec);
        varint(3 << 3, &mut spec);
        varint(1, &mut spec);
        bytes_field(3, &spec, &mut out);
        out
    }

    #[test]
    fn splits_the_way_sentencepiece_does() {
        let model = SentencePiece::from_bytes(&model()).unwrap();
        // The most likely split, with spaces as `▁` and a dummy one first.
        assert_eq!(
            model.pieces("  hello   world ").unwrap(),
            ["▁hello", "▁world"]
        );
        // Unknown characters come out as themselves; the caller maps them to `<unk>`.
        let pieces = model.pieces("hello ☃").unwrap();
        assert_eq!(pieces[0], "▁hello");
        assert!(pieces.iter().any(|piece| piece.contains('☃')));
        assert!(model.pieces("   ").unwrap().is_empty());
        assert_eq!(join(&["▁Hello", ",", "▁world"]), "Hello, world");
    }

    #[test]
    fn damaged_files_are_errors() {
        assert!(SentencePiece::from_bytes(&[0x0a, 0x05, 0x01]).is_err());
        assert!(SentencePiece::from_bytes(&[]).is_err());
    }
}
