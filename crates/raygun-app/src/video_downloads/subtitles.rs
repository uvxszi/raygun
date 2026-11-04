//! Which subtitle tracks a download saves. The picker lists a link's tracks with the remembered
//! languages ticked; what was picked is remembered per source (`local_value`
//! `video.downloadSubtitles.<source>`), and the device's language is ticked unless it was left
//! out before. Downloads queued without a picker (several episodes at once, exports) take the
//! same choice.

use std::collections::HashSet;

use raygun_model::VideoSubtitle;
use serde::{Deserialize, Serialize};

use crate::stremio::protocol::language_names;

/// The `local_value` key of a source's remembered choice.
pub fn memory_key(source: &str) -> String {
    format!("video.downloadSubtitles.{source}")
}

/// Languages picked and left out at a source, as two-letter codes where one is known.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleMemory {
    #[serde(default)]
    pub chosen: Vec<String>,
    #[serde(default)]
    pub declined: Vec<String>,
}

/// Codes `language_names` knows the other spellings of.
const CODES: &[&str] = &[
    "en", "es", "fr", "de", "pt", "it", "ja", "ru", "zh", "ko", "ar", "nl", "pl", "tr", "sv", "no",
    "da", "fi", "cs", "el", "he", "hu", "ro", "hi", "id", "vi", "th", "uk", "bg", "hr", "sr", "sk",
    "sl", "fa", "ms",
];

/// A track's language as one code: `en` for "en-US", "eng" or "English"; other values lowercased
/// without their region. `None` for a blank one.
pub fn language_code(lang: &str) -> Option<String> {
    let lower = lang.trim().to_lowercase();
    if lower.is_empty() {
        return None;
    }
    let base = lower.split(['-', '_']).next().unwrap_or(&lower).trim();
    let code = CODES
        .iter()
        .find(|code| {
            **code == base
                || language_names(code).contains(&base)
                || language_names(code).contains(&lower.as_str())
        })
        .map_or_else(|| base.to_owned(), |code| (*code).to_owned());
    // Norwegian Bokmål is Norwegian here.
    Some(if code == "nb" { "no".into() } else { code })
}

fn track_code(track: &VideoSubtitle) -> Option<String> {
    track.lang.as_deref().and_then(language_code)
}

/// Whether a track is ticked: its language was picked at this source, or it's the device's
/// language and wasn't left out.
pub fn is_selected(memory: &SubtitleMemory, track: &VideoSubtitle, device: &str) -> bool {
    let Some(code) = track_code(track) else {
        return false;
    };
    if memory.chosen.contains(&code) {
        return true;
    }
    language_code(device).is_some_and(|device| device == code) && !memory.declined.contains(&code)
}

/// The addresses ticked at first, in the tracks' order.
pub fn defaults(memory: &SubtitleMemory, tracks: &[VideoSubtitle], device: &str) -> Vec<String> {
    tracks
        .iter()
        .filter(|track| is_selected(memory, track, device))
        .map(|track| track.url.clone())
        .collect()
}

/// Remembers a pick among `shown` tracks: each language with a ticked track is picked, each whose
/// tracks were all left unticked is left out. Languages not shown keep what they had.
pub fn remember(memory: &mut SubtitleMemory, shown: &[VideoSubtitle], chosen: &HashSet<String>) {
    let mut picked = HashSet::new();
    let mut seen = HashSet::new();
    for track in shown {
        let Some(code) = track_code(track) else {
            continue;
        };
        if chosen.contains(&track.url) {
            picked.insert(code.clone());
        }
        seen.insert(code);
    }
    for code in seen {
        let (add, drop) = if picked.contains(&code) {
            (&mut memory.chosen, &mut memory.declined)
        } else {
            (&mut memory.declined, &mut memory.chosen)
        };
        drop.retain(|known| *known != code);
        if !add.contains(&code) {
            add.push(code);
        }
    }
}

/// The tracks a download saves: those ticked among `shown` (when a picker showed them), and the
/// rest (tracks found after the pick, such as subtitle addons') by the remembered languages.
pub fn apply(
    memory: &SubtitleMemory,
    tracks: Vec<VideoSubtitle>,
    shown: &HashSet<String>,
    chosen: Option<&HashSet<String>>,
    device: &str,
) -> Vec<VideoSubtitle> {
    tracks
        .into_iter()
        .filter(|track| match chosen {
            Some(chosen) if shown.contains(&track.url) => chosen.contains(&track.url),
            _ => is_selected(memory, track, device),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(url: &str, lang: &str) -> VideoSubtitle {
        VideoSubtitle {
            url: url.into(),
            lang: (!lang.is_empty()).then(|| lang.into()),
            label: None,
            format: None,
        }
    }

    #[test]
    fn languages_read_as_one_code() {
        assert_eq!(language_code("en-US").as_deref(), Some("en"));
        assert_eq!(language_code("eng").as_deref(), Some("en"));
        assert_eq!(language_code("English").as_deref(), Some("en"));
        assert_eq!(language_code("pob").as_deref(), Some("pt"));
        assert_eq!(language_code("nb-NO").as_deref(), Some("no"));
        assert_eq!(language_code("tlh").as_deref(), Some("tlh"));
        assert_eq!(language_code("  "), None);
    }

    #[test]
    fn the_device_language_is_ticked_until_it_is_left_out() {
        let tracks = vec![
            track("https://s/en.srt", "eng"),
            track("https://s/fr.srt", "fr"),
            track("https://s/x.srt", ""),
        ];
        let mut memory = SubtitleMemory::default();
        assert_eq!(
            defaults(&memory, &tracks, "en-GB"),
            vec!["https://s/en.srt"]
        );

        // English left out, French picked: both remembered for this source.
        let chosen: HashSet<String> = ["https://s/fr.srt".to_owned()].into();
        remember(&mut memory, &tracks, &chosen);
        assert_eq!(memory.chosen, vec!["fr"]);
        assert_eq!(memory.declined, vec!["en"]);
        assert_eq!(
            defaults(&memory, &tracks, "en-GB"),
            vec!["https://s/fr.srt"]
        );

        // Picking English again takes it off the left-out list.
        let chosen: HashSet<String> = ["https://s/en.srt".to_owned()].into();
        remember(&mut memory, &tracks[..1], &chosen);
        assert_eq!(memory.declined, Vec::<String>::new());
        assert_eq!(
            defaults(&memory, &tracks, "en"),
            vec!["https://s/en.srt", "https://s/fr.srt"]
        );
        let json = serde_json::to_string(&memory).unwrap();
        assert_eq!(
            serde_json::from_str::<SubtitleMemory>(&json).unwrap(),
            memory
        );
    }

    #[test]
    fn a_pick_keeps_its_tracks_and_later_ones_follow_the_memory() {
        let shown_tracks = vec![
            track("https://s/1.srt", "en"),
            track("https://s/2.srt", "en"),
        ];
        let found_later = track("https://addon/3.srt", "en");
        let other = track("https://addon/4.srt", "de");
        let shown: HashSet<String> = shown_tracks.iter().map(|track| track.url.clone()).collect();
        let chosen: HashSet<String> = ["https://s/2.srt".to_owned()].into();
        let mut memory = SubtitleMemory::default();
        remember(&mut memory, &shown_tracks, &chosen);
        let mut all = shown_tracks.clone();
        all.push(found_later.clone());
        all.push(other);
        let saved: Vec<String> = apply(&memory, all, &shown, Some(&chosen), "fr")
            .into_iter()
            .map(|track| track.url)
            .collect();
        assert_eq!(saved, vec!["https://s/2.srt", "https://addon/3.srt"]);

        // Without a picker (a bulk download), the memory alone decides.
        let saved = apply(
            &memory,
            vec![found_later, track("https://s/9.srt", "fr")],
            &HashSet::new(),
            None,
            "fr",
        );
        assert_eq!(saved.len(), 2);
    }
}
