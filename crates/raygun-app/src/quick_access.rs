//! Quick access (M46.41): what's added to the library is pinned to the sidebar (and the phone's
//! Your library row) as an automatic pin, and unpinned when it leaves the library. Automatic pins
//! beyond `RailPrefs::limit` go oldest first; pins made by hand are never touched here.

use std::collections::BTreeMap;

use raygun_db::titles::TitleRow;
use raygun_model::{RailPin, SettingChange};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::Mutex;

use crate::events::SettingsChangedEvent;
use crate::startup::AppDb;

/// One change at a time: two additions at once would each save the other's pins away.
static CHANGES: Mutex<()> = Mutex::const_new(());

/// Something with a page, to pin.
pub struct QuickPin {
    /// As `RailPin::kind`.
    pub kind: &'static str,
    pub to: String,
    pub search: BTreeMap<String, String>,
    pub name: String,
    pub artwork_url: Option<String>,
}

fn now_s() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| u32::try_from(elapsed.as_secs()).unwrap_or(u32::MAX))
        .unwrap_or_default()
}

fn search(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

/// A library title's page (comics, books, videos; a Stremio title's own page).
pub fn title_pin(row: &TitleRow) -> QuickPin {
    let fields = &row.fields;
    let (kind, to, search) = match fields.kind.as_str() {
        "novel" => (
            "novel",
            "/novel/title",
            search(&[("source", &fields.source_id), ("id", &fields.remote_id)]),
        ),
        "video" if fields.source_id == "stremio" => {
            let (kind, id) = fields
                .remote_id
                .split_once(':')
                .unwrap_or(("movie", &fields.remote_id));
            (
                "video",
                "/watch/title",
                search(&[("type", kind), ("id", id)]),
            )
        }
        "video" => (
            "video",
            "/watch/video",
            search(&[("source", &fields.source_id), ("id", &fields.remote_id)]),
        ),
        _ => (
            "manga",
            "/manga/title",
            search(&[("source", &fields.source_id), ("id", &fields.remote_id)]),
        ),
    };
    QuickPin {
        kind,
        to: to.to_owned(),
        search,
        name: fields.title.clone(),
        artwork_url: fields.cover.clone(),
    }
}

/// A page with one `id` parameter (a playlist, a podcast, an audiobook).
pub fn page_pin(
    kind: &'static str,
    to: &str,
    id: &str,
    name: &str,
    art: Option<String>,
) -> QuickPin {
    QuickPin {
        kind,
        to: to.to_owned(),
        search: search(&[("id", id)]),
        name: name.to_owned(),
        artwork_url: art,
    }
}

/// Pins what was added to the library, unless quick access is set not to.
pub async fn add(app: &AppHandle, pin: QuickPin) {
    change(app, |rail| {
        if !rail.auto_add
            || rail
                .pins
                .iter()
                .any(|other| other.to == pin.to && other.search == pin.search)
        {
            return false;
        }
        rail.pins.push(RailPin {
            kind: pin.kind.to_owned(),
            to: pin.to,
            search: pin.search,
            name: pin.name,
            artwork_url: pin.artwork_url,
            pinned_at: now_s(),
            played_at: 0,
            opened_at: 0,
            auto: true,
        });
        // Past the limit, the oldest automatic pins go.
        let limit = rail.limit as usize;
        let mut automatic: Vec<(u32, usize)> = rail
            .pins
            .iter()
            .enumerate()
            .filter(|(_, pin)| pin.auto)
            .map(|(index, pin)| (pin.pinned_at, index))
            .collect();
        if automatic.len() > limit {
            automatic.sort();
            let gone: Vec<usize> = automatic
                .iter()
                .take(automatic.len() - limit)
                .map(|(_, index)| *index)
                .collect();
            let mut index = 0;
            rail.pins.retain(|_| {
                let keep = !gone.contains(&index);
                index += 1;
                keep
            });
        }
        rail.pins.len() <= raygun_model::RAIL_MAX_PINS
    })
    .await;
}

/// Unpins what left the library, when it was pinned automatically.
pub async fn remove(app: &AppHandle, to: &str, search: &BTreeMap<String, String>) {
    change(app, |rail| {
        let before = rail.pins.len();
        rail.pins
            .retain(|pin| !(pin.auto && pin.to == to && &pin.search == search));
        rail.pins.len() != before
    })
    .await;
}

/// Changes the rail and saves it when `edit` says it changed; the UI hears of it.
async fn change(app: &AppHandle, edit: impl FnOnce(&mut raygun_model::RailPrefs) -> bool) {
    let _turn = CHANGES.lock().await;
    let db = app.state::<AppDb>().0.clone();
    let Ok(settings) = crate::settings::load(&db).await else {
        return;
    };
    let mut rail = settings.rail;
    if !edit(&mut rail) {
        return;
    }
    match crate::settings::update(&db, SettingChange::Rail(rail)).await {
        Ok(_) => {
            let _ = SettingsChangedEvent("rail".into()).emit(app);
        }
        Err(error) => tracing::warn!(%error, "could not change quick access"),
    }
}
