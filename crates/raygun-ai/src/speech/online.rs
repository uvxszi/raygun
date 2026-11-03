//! Online transcription: any OpenAI-compatible `POST {base}/audio/transcriptions` (OpenAI,
//! Groq, a local server…), sent one window as a WAV file and answered with timed segments.

use serde_json::Value;

use super::{SAMPLE_RATE, Segment, SpeechError};

/// Longest provider error text kept in a message.
const MAX_ERROR_CHARS: usize = 300;

/// 16-bit mono PCM WAV of 16 kHz samples.
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// Sends one window; `duration_ms` covers an answer that has text but no segments.
pub async fn transcribe_online(
    client: &reqwest::Client,
    base_url: &str,
    key: Option<&str>,
    model: &str,
    samples: &[f32],
    language: Option<&str>,
) -> Result<Vec<Segment>, SpeechError> {
    let duration_ms = (samples.len() as u64 * 1000 / SAMPLE_RATE as u64) as u32;
    let boundary = format!("raygun-{:016x}", rand_tag(samples));
    let mut body = Vec::new();
    let mut field = |name: &str, value: &str| {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    };
    field("model", model);
    field("response_format", "verbose_json");
    field("timestamp_granularities[]", "segment");
    if let Some(language) = language {
        field("language", language);
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&wav_bytes(samples));
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let url = format!("{}/audio/transcriptions", base_url.trim_end_matches('/'));
    let mut request = client
        .post(url)
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body);
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let response = request
        .send()
        .await
        .map_err(|error| SpeechError::Network(error.to_string()))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| SpeechError::Network(error.to_string()))?;
    if !status.is_success() {
        return Err(SpeechError::Provider {
            status: status.as_u16(),
            message: provider_message(&text),
        });
    }
    let json: Value = serde_json::from_str(&text).map_err(|_| SpeechError::Provider {
        status: status.as_u16(),
        message: "the answer wasn't JSON".into(),
    })?;
    Ok(parse_answer(&json, duration_ms))
}

fn parse_answer(json: &Value, duration_ms: u32) -> Vec<Segment> {
    let millis = |value: Option<&Value>| {
        value
            .and_then(Value::as_f64)
            .map(|seconds| ((seconds.max(0.0) * 1000.0) as u32).min(duration_ms))
    };
    let segments: Vec<Segment> = json
        .get("segments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|segment| {
            let text = segment.get("text")?.as_str()?.trim();
            (!text.is_empty()).then(|| Segment {
                start_ms: millis(segment.get("start")).unwrap_or(0),
                end_ms: millis(segment.get("end")).unwrap_or(duration_ms),
                text: text.to_owned(),
            })
        })
        .collect();
    if !segments.is_empty() {
        return segments;
    }
    json.get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| {
            vec![Segment {
                start_ms: 0,
                end_ms: duration_ms,
                text: text.to_owned(),
            }]
        })
        .unwrap_or_default()
}

fn provider_message(text: &str) -> String {
    let message = serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|json| {
            json.pointer("/error/message")
                .or_else(|| json.get("error"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| text.to_owned());
    message.chars().take(MAX_ERROR_CHARS).collect()
}

/// A boundary tag that won't appear in the audio by chance.
fn rand_tag(samples: &[f32]) -> u64 {
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        return samples.len() as u64 ^ 0x9E37_79B9_7F4A_7C15;
    }
    u64::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_timed_segments() {
        let answer = json!({
            "text": "Hello there. General Kenobi.",
            "segments": [
                {"start": 0.0, "end": 1.5, "text": " Hello there."},
                {"start": 1.5, "end": 40.0, "text": " General Kenobi."}
            ]
        });
        let segments = parse_answer(&answer, 30_000);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "Hello there.");
        assert_eq!(segments[1].end_ms, 30_000);
    }

    #[test]
    fn plain_text_covers_the_window() {
        let segments = parse_answer(&json!({"text": "Just words"}), 12_000);
        assert_eq!(
            segments,
            vec![Segment {
                start_ms: 0,
                end_ms: 12_000,
                text: "Just words".into()
            }]
        );
    }

    #[test]
    fn wav_has_a_header_and_two_bytes_a_sample() {
        let bytes = wav_bytes(&[0.0, 1.0, -1.0]);
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(bytes.len(), 44 + 6);
        assert_eq!(i16::from_le_bytes([bytes[46], bytes[47]]), i16::MAX);
    }
}
