//! Reading novels aloud (M33) with the phone's text-to-speech engine, through the native audio
//! plugin on Android. On desktop (M46.6) the webview speaks (`speechSynthesis` with the system's
//! voices): Rust sends it `SpeechCommandEvent`s and it reports back through `tts_report` in the
//! same JSON as Android's engine. Progress comes back as `TtsEvent`s.

use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(target_os = "android"))]
use raygun_model::{SpeechCommand, SpeechLine};
use raygun_model::{TtsParagraph, TtsState, TtsUpdate};
use tauri::{AppHandle, Manager};
use tauri_plugin_raygun_audio::SpeechItem;
use tauri_specta::Event;

/// Whether the webview's speech engine can speak (desktop).
#[cfg_attr(target_os = "android", allow(dead_code))]
struct SpeechAvailable(AtomicBool);

/// Longest piece handed to the engine at once (Android accepts about 4000 characters).
const MAX_UTTERANCE: usize = 3_000;

/// The paragraphs as utterances: long ones split at sentence ends, and the very last one marked
/// `:end` so the engine reports when everything was read.
pub fn utterances(paragraphs: &[TtsParagraph]) -> Vec<SpeechItem> {
    let mut items = Vec::new();
    for paragraph in paragraphs {
        for piece in split(&paragraph.text) {
            items.push(SpeechItem {
                id: paragraph.id.to_string(),
                text: piece,
            });
        }
    }
    if let Some(last) = items.last_mut() {
        last.id.push_str(":end");
    }
    items
}

fn split(text: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let mut pieces = Vec::new();
    let mut current = String::new();
    for sentence in text.split_inclusive(['.', '!', '?', '。', '！', '？']) {
        if current.chars().count() + sentence.chars().count() > MAX_UTTERANCE && !current.is_empty()
        {
            pieces.push(std::mem::take(&mut current).trim().to_owned());
        }
        // A single sentence longer than the limit is cut at the limit.
        let mut sentence: Vec<char> = sentence.chars().collect();
        while sentence.len() > MAX_UTTERANCE {
            let rest = sentence.split_off(MAX_UTTERANCE);
            pieces.push(sentence.iter().collect::<String>().trim().to_owned());
            sentence = rest;
        }
        current.extend(sentence);
    }
    if !current.trim().is_empty() {
        pieces.push(current.trim().to_owned());
    }
    pieces
}

/// Parses a progress report from the engine.
pub fn parse_update(json: &str) -> Option<TtsUpdate> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let state = match value.get("state")?.as_str()? {
        "speaking" => TtsState::Speaking,
        "ended" => TtsState::Ended,
        _ => TtsState::Error,
    };
    let paragraph = value.get("paragraph")?.as_u64()? as u32;
    Some(TtsUpdate { state, paragraph })
}

#[cfg(target_os = "android")]
fn engine(
    app: &AppHandle,
) -> Option<tauri::State<'_, tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>> {
    use tauri::Manager;
    app.try_state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
}

/// Sends the engine's progress to the UI as `TtsEvent`s.
pub fn install(app: &AppHandle) {
    app.manage(SpeechAvailable(AtomicBool::new(false)));
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        tauri_plugin_raygun_audio::set_tts_handler(move |json| report(&app, &json));
    }
}

/// A progress report from an engine (Android's over JNI, the webview's through `tts_report`).
pub fn report(app: &AppHandle, json: &str) {
    if let Some(update) = parse_update(json) {
        let _ = crate::events::TtsEvent(update).emit(app);
    }
}

/// What the webview's engine says when it starts: whether `speechSynthesis` is there.
pub fn set_available(app: &AppHandle, available: bool) {
    if let Some(state) = app.try_state::<SpeechAvailable>() {
        state.0.store(available, Ordering::Relaxed);
    }
}

/// The pieces as the webview's engine takes them.
#[cfg(not(target_os = "android"))]
fn lines(items: Vec<SpeechItem>) -> Vec<SpeechLine> {
    items
        .into_iter()
        .map(|item| SpeechLine {
            id: item.id,
            text: item.text,
        })
        .collect()
}

pub async fn available(app: &AppHandle) -> bool {
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            engine(&app).is_some_and(|engine| engine.tts_available())
        })
        .await
        .unwrap_or(false)
    }
    #[cfg(not(target_os = "android"))]
    {
        app.try_state::<SpeechAvailable>()
            .is_some_and(|state| state.0.load(Ordering::Relaxed))
    }
}

pub async fn speak(
    app: &AppHandle,
    paragraphs: Vec<TtsParagraph>,
    rate: f64,
) -> Result<(), String> {
    let items = utterances(&paragraphs);
    if items.is_empty() {
        return Err("There is nothing to read".into());
    }
    let rate = rate.clamp(0.5, 2.0);
    crate::playback::other_media_started(app).await;
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            engine(&app)
                .ok_or_else(|| "Text-to-speech isn't available".to_owned())?
                .tts_speak(&items, rate)
        })
        .await
        .map_err(|error| error.to_string())?
    }
    #[cfg(not(target_os = "android"))]
    {
        if !available(app).await {
            return Err("This system has no voices to read aloud with".into());
        }
        crate::events::SpeechCommandEvent(SpeechCommand::Speak {
            items: lines(items),
            rate,
        })
        .emit(app)
        .map_err(|error| error.to_string())
    }
}

pub async fn stop(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || match engine(&app) {
            Some(engine) => engine.tts_stop(),
            None => Ok(()),
        })
        .await
        .map_err(|error| error.to_string())?
    }
    #[cfg(not(target_os = "android"))]
    {
        crate::events::SpeechCommandEvent(SpeechCommand::Stop)
            .emit(app)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(id: u32, text: &str) -> TtsParagraph {
        TtsParagraph {
            id,
            text: text.into(),
        }
    }

    #[test]
    fn marks_the_last_utterance_and_splits_long_paragraphs() {
        let long = "A sentence that goes on. ".repeat(200);
        let items = utterances(&[
            paragraph(3, "Short."),
            paragraph(4, &long),
            paragraph(5, "  "),
        ]);
        assert_eq!(items[0].id, "3");
        assert!(items.len() > 2, "the long paragraph is split");
        assert!(
            items
                .iter()
                .all(|item| item.text.chars().count() <= MAX_UTTERANCE)
        );
        assert!(items[1..items.len() - 1].iter().all(|item| item.id == "4"));
        assert_eq!(
            items.last().unwrap().id,
            "4:end",
            "blank paragraphs are skipped"
        );
        let unbroken = "x".repeat(MAX_UTTERANCE * 2 + 5);
        assert_eq!(split(&unbroken).len(), 3);
    }

    #[test]
    fn reads_engine_reports() {
        assert_eq!(
            parse_update(r#"{"state":"speaking","paragraph":7}"#),
            Some(TtsUpdate {
                state: TtsState::Speaking,
                paragraph: 7
            })
        );
        assert_eq!(
            parse_update(r#"{"state":"ended","paragraph":9}"#)
                .unwrap()
                .state,
            TtsState::Ended
        );
        assert_eq!(parse_update("nope"), None);
    }
}
