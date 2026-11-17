//! Quick access rows as older versions stored them, read the way quick access is now (the
//! settings read them; `quick_access` in the app keeps the rest).

use raygun_model::{RAIL_PLAYED_KEPT, RAIL_VERSION, RailPin, RailPlayed, RailPrefs};

/// Kinds the audio library lists by itself; they're never pinned automatically.
pub const AUDIO_LIBRARY: &[&str] = &[
    "favorites",
    "downloaded",
    "localFiles",
    "playlist",
    "album",
    "artist",
    "podcast",
    "audiobook",
];

/// The fixed blocks, in the order they keep among equals.
pub const FIXED: [&str; 3] = ["favorites", "downloaded", "localFiles"];

/// Older versions pinned the first run's blocks and playlists at once, within this many seconds.
const SEED_WINDOW_S: u32 = 2;

/// A stored row's JSON before it's read: older versions' orders, the custom one and Recently
/// opened, are Recently played now.
pub fn migrate_row(value: &str) -> String {
    let Ok(serde_json::Value::Object(mut fields)) = serde_json::from_str(value) else {
        return value.to_owned();
    };
    if matches!(
        fields.get("order").and_then(serde_json::Value::as_str),
        Some("custom" | "opened")
    ) {
        fields.insert("order".to_owned(), "played".into());
    }
    serde_json::Value::Object(fields).to_string()
}

/// A row of an older version (`version` 0) as quick access is now:
/// - its pins' played stamps move to `played`;
/// - automatic pins of the audio library go, since it lists itself;
/// - the first run's pins (Favorites, Downloaded, Local files and the playlists there were, all
///   pinned in the same moment) go too, so they're sorted like the rest of the library;
/// - every other pin, made by hand, stays pinned, in its order.
///
/// Its custom order and Recently opened are Recently played by then ([`migrate_row`]).
pub fn migrated(mut rail: RailPrefs) -> RailPrefs {
    if rail.version >= RAIL_VERSION {
        return rail;
    }
    for pin in &rail.pins {
        if pin.played_at == 0 {
            continue;
        }
        let known = rail
            .played
            .iter_mut()
            .find(|stamp| stamp.to == pin.to && stamp.search == pin.search);
        match known {
            Some(stamp) => stamp.at = stamp.at.max(pin.played_at),
            None => rail.played.push(RailPlayed {
                to: pin.to.clone(),
                search: pin.search.clone(),
                at: pin.played_at,
            }),
        }
    }
    rail.played.sort_by_key(|stamp| std::cmp::Reverse(stamp.at));
    rail.played.truncate(RAIL_PLAYED_KEPT);

    let seeded_at = rail
        .pins
        .iter()
        .filter(|pin| !pin.auto && FIXED.contains(&pin.kind.as_str()))
        .map(|pin| pin.pinned_at)
        .min()
        .filter(|_| rail.seeded);
    let first_run = |pin: &RailPin| {
        seeded_at.is_some_and(|at| {
            !pin.auto
                && (FIXED.contains(&pin.kind.as_str()) || pin.kind == "playlist")
                && pin.pinned_at.abs_diff(at) <= SEED_WINDOW_S
        })
    };
    rail.pins.retain(|pin| {
        let listed_anyway = pin.auto && AUDIO_LIBRARY.contains(&pin.kind.as_str());
        !listed_anyway && !first_run(pin)
    });
    for pin in &mut rail.pins {
        pin.played_at = 0;
        pin.opened_at = 0;
    }
    rail.version = RAIL_VERSION;
    rail
}
