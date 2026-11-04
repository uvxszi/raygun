//! A Shosetsu script's search filters (kotlin-lib's `Filter`, GPL-3.0) as Raygun's source
//! filters, and the values the app sends back as the script's filter data, keyed by filter id.
//!
//! Dropdowns and radio groups become selects (the choice's index is the value, as Shosetsu
//! passes it), switches and checkboxes toggles, and a group of checkboxes or tri-states (genres,
//! usually) one set of include/exclude chips. Text filters have no Raygun form and keep their
//! default. A script with more than one listing also gets a "Listing" select, which picks the
//! listing an empty search shows, as Shosetsu's listing picker does.

use mlua::{Table, Value};
use serde_json::{Value as Json, json};

/// The key of the select that picks a listing.
pub const LISTING_KEY: &str = "listing";

/// One of the script's filters.
#[derive(Debug, Clone, PartialEq)]
pub enum Spec {
    /// A text or password field (its `state`: "" unless the script set one).
    Text {
        id: i64,
        name: String,
        state: String,
    },
    /// A switch or checkbox (false unless set).
    Toggle { id: i64, name: String, state: bool },
    /// Ignored (0), included (1) or excluded (2).
    TriState { id: i64, name: String, state: i64 },
    /// A dropdown or radio group: the chosen index (0 unless set).
    Choice {
        id: i64,
        name: String,
        choices: Vec<String>,
        state: usize,
    },
    /// A list or group of filters, under a name.
    Group { name: String, filters: Vec<Spec> },
    /// A header or separator: nothing to choose.
    Label,
}

/// The filters a script declares in `searchFilters`.
pub fn read(filters: &Table) -> Vec<Spec> {
    filters
        .clone()
        .sequence_values::<Value>()
        .filter_map(Result::ok)
        .filter_map(|filter| match filter {
            Value::Table(filter) => Some(spec(&filter)),
            _ => None,
        })
        .collect()
}

fn spec(filter: &Table) -> Spec {
    let kind: String = filter.get("kind").unwrap_or_default();
    let id: Option<i64> = filter.get("id").ok().flatten();
    let name: String = filter.get("name").unwrap_or_default();
    match (kind.as_str(), id) {
        ("list" | "group", _) => Spec::Group {
            name,
            filters: filter
                .get::<Table>("filters")
                .map(|inner| read(&inner))
                .unwrap_or_default(),
        },
        ("text" | "password", Some(id)) => Spec::Text {
            id,
            name,
            state: filter
                .get::<Option<String>>("state")
                .ok()
                .flatten()
                .unwrap_or_default(),
        },
        ("switch" | "checkbox", Some(id)) => Spec::Toggle {
            id,
            name,
            state: filter
                .get::<Option<bool>>("state")
                .ok()
                .flatten()
                .unwrap_or(false),
        },
        ("tristate", Some(id)) => Spec::TriState {
            id,
            name,
            state: filter
                .get::<Option<i64>>("state")
                .ok()
                .flatten()
                .unwrap_or(0),
        },
        ("dropdown" | "radio", Some(id)) => Spec::Choice {
            id,
            name,
            state: filter
                .get::<Option<i64>>("state")
                .ok()
                .flatten()
                .and_then(|state| usize::try_from(state).ok())
                .unwrap_or(0),
            choices: filter
                .get::<Table>("choices")
                .map(|choices| {
                    choices
                        .sequence_values::<Value>()
                        .filter_map(Result::ok)
                        .map(|choice| match choice {
                            Value::String(text) => text.to_string_lossy(),
                            Value::Integer(number) => number.to_string(),
                            Value::Number(number) => number.to_string(),
                            _ => String::new(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        },
        _ => Spec::Label,
    }
}

/// A group whose filters are all checkboxes or tri-states: one set of chips.
fn chips(filters: &[Spec]) -> Option<Vec<(i64, &str)>> {
    let items: Vec<(i64, &str)> = filters
        .iter()
        .map(|filter| match filter {
            Spec::Toggle { id, name, .. } | Spec::TriState { id, name, .. } => {
                Some((*id, name.as_str()))
            }
            _ => None,
        })
        .collect::<Option<_>>()?;
    (!items.is_empty()).then_some(items)
}

fn group_key(items: &[(i64, &str)]) -> String {
    format!("g{}", items[0].0)
}

/// The filters as Raygun's `SourceFilter` JSON (`select`, `toggle`, `tags`), after a listing
/// picker when there are several `listings` (`shown` is the one an empty search shows, 1-based).
pub fn describe(specs: &[Spec], listings: &[String], shown: usize, out: &mut Vec<Json>) {
    if listings.len() > 1 {
        out.push(json!({
            "key": LISTING_KEY,
            "label": "Listing",
            "kind": "select",
            "options": listings
                .iter()
                .enumerate()
                .map(|(index, name)| json!({ "value": (index + 1).to_string(), "label": name }))
                .collect::<Vec<_>>(),
            "default": shown.to_string(),
        }));
    }
    choices(specs, out);
}

fn choices(specs: &[Spec], out: &mut Vec<Json>) {
    for spec in specs {
        match spec {
            Spec::Toggle { id, name, state } => out.push(json!({
                "key": id.to_string(), "label": name, "kind": "toggle", "default": state,
            })),
            Spec::TriState { id, name, .. } => out.push(json!({
                "key": id.to_string(), "label": name, "kind": "tags",
                "options": [{ "value": id.to_string(), "label": name }],
            })),
            Spec::Choice {
                id,
                name,
                choices,
                state,
            } if !choices.is_empty() => out.push(json!({
                "key": id.to_string(),
                "label": name,
                "kind": "select",
                "options": choices
                    .iter()
                    .enumerate()
                    .map(|(index, label)| json!({ "value": index.to_string(), "label": label }))
                    .collect::<Vec<_>>(),
                "default": (if *state < choices.len() { *state } else { 0 }).to_string(),
            })),
            Spec::Group { name, filters } => match chips(filters) {
                Some(items) => out.push(json!({
                    "key": group_key(&items),
                    "label": name,
                    "kind": "tags",
                    "options": items
                        .iter()
                        .map(|(id, label)| json!({ "value": id.to_string(), "label": label }))
                        .collect::<Vec<_>>(),
                })),
                None => choices(filters, out),
            },
            Spec::Text { .. } | Spec::Choice { .. } | Spec::Label => {}
        }
    }
}

/// A script's `settings` (built from the same filters) as extension settings the app's form
/// shows: a dropdown's options are its labels, a switch is a boolean, a text field text.
pub fn settings(specs: &[Spec], out: &mut Vec<crate::manifest::SettingDef>) {
    use crate::manifest::{SettingDef, SettingKind};
    for spec in specs {
        let (key, kind, label, options, default) = match spec {
            Spec::Choice {
                id,
                name,
                choices,
                state,
            } if !choices.is_empty() => (
                id,
                SettingKind::Select,
                name,
                choices.clone(),
                json!(choices.get(*state).unwrap_or(&choices[0])),
            ),
            Spec::Toggle { id, name, state } => {
                (id, SettingKind::Boolean, name, Vec::new(), json!(state))
            }
            Spec::Text { id, name, state } => {
                (id, SettingKind::Text, name, Vec::new(), json!(state))
            }
            Spec::Group { filters, .. } => {
                settings(filters, out);
                continue;
            }
            _ => continue,
        };
        out.push(SettingDef {
            key: key.to_string(),
            kind,
            label: Some(label.clone()).filter(|label| !label.is_empty()),
            options,
            default: Some(default),
        });
    }
}

/// A stored setting's value as the script reads it: a dropdown's index, a switch's flag, text.
pub fn setting_value(specs: &[Spec], key: &str, value: &Json) -> Option<(i64, Json)> {
    for spec in specs {
        match spec {
            Spec::Choice { id, choices, .. } if id.to_string() == key => {
                let label = value.as_str()?;
                let index = choices.iter().position(|choice| choice == label)?;
                return Some((*id, json!(index)));
            }
            Spec::Toggle { id, .. } if id.to_string() == key => {
                return Some((*id, json!(value.as_bool()?)));
            }
            Spec::Text { id, .. } if id.to_string() == key => {
                return Some((*id, json!(value.as_str()?)));
            }
            Spec::Group { filters, .. } => {
                if let Some(found) = setting_value(filters, key, value) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

/// Every filter's default, by id: what Shosetsu hands a script before anything is chosen.
pub fn defaults(specs: &[Spec], data: &Table) -> mlua::Result<()> {
    for spec in specs {
        match spec {
            Spec::Text { id, state, .. } => data.set(*id, state.as_str())?,
            Spec::Toggle { id, state, .. } => data.set(*id, *state)?,
            Spec::TriState { id, state, .. } => data.set(*id, *state)?,
            Spec::Choice { id, state, .. } => data.set(*id, *state)?,
            Spec::Group { filters, .. } => defaults(filters, data)?,
            Spec::Label => {}
        }
    }
    Ok(())
}

/// The listing a search's filters pick (1-based), when they pick one.
pub fn chosen_listing(chosen: &serde_json::Map<String, Json>, count: usize) -> Option<usize> {
    chosen
        .get(LISTING_KEY)?
        .as_str()?
        .parse::<usize>()
        .ok()
        .filter(|index| (1..=count).contains(index))
}

/// Puts the app's chosen values (`{ key: bool | "index" | { include, exclude } }`) into `data`.
pub fn apply(
    specs: &[Spec],
    chosen: &serde_json::Map<String, Json>,
    data: &Table,
) -> mlua::Result<()> {
    let ids = |value: &Json, list: &str| -> Vec<i64> {
        value
            .get(list)
            .and_then(Json::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str()?.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    };
    for spec in specs {
        match spec {
            Spec::Toggle { id, .. } => {
                if let Some(on) = chosen.get(&id.to_string()).and_then(Json::as_bool) {
                    data.set(*id, on)?;
                }
            }
            Spec::TriState { id, .. } => {
                if let Some(value) = chosen.get(&id.to_string()) {
                    let state = if ids(value, "include").contains(id) {
                        1
                    } else if ids(value, "exclude").contains(id) {
                        2
                    } else {
                        0
                    };
                    data.set(*id, state)?;
                }
            }
            Spec::Choice { id, choices, .. } => {
                if let Some(index) = chosen
                    .get(&id.to_string())
                    .and_then(Json::as_str)
                    .and_then(|index| index.parse::<usize>().ok())
                    .filter(|index| *index < choices.len())
                {
                    data.set(*id, index)?;
                }
            }
            Spec::Group { filters, .. } => match chips(filters) {
                Some(items) => {
                    let Some(value) = chosen.get(&group_key(&items)) else {
                        continue;
                    };
                    let (include, exclude) = (ids(value, "include"), ids(value, "exclude"));
                    for filter in filters {
                        match filter {
                            Spec::Toggle { id, .. } => data.set(*id, include.contains(id))?,
                            Spec::TriState { id, .. } => {
                                let state = if include.contains(id) {
                                    1
                                } else if exclude.contains(id) {
                                    2
                                } else {
                                    0
                                };
                                data.set(*id, state)?;
                            }
                            _ => {}
                        }
                    }
                }
                None => apply(filters, chosen, data)?,
            },
            Spec::Text { .. } | Spec::Label => {}
        }
    }
    Ok(())
}
