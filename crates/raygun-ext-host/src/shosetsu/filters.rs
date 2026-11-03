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
    /// A text or password field (default "").
    Text { id: i64 },
    /// A switch or checkbox (default false).
    Toggle { id: i64, name: String },
    /// Ignored (0), included (1) or excluded (2).
    TriState { id: i64, name: String },
    /// A dropdown or radio group: the chosen index (default 0).
    Choice {
        id: i64,
        name: String,
        choices: Vec<String>,
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
        ("text" | "password", Some(id)) => Spec::Text { id },
        ("switch" | "checkbox", Some(id)) => Spec::Toggle { id, name },
        ("tristate", Some(id)) => Spec::TriState { id, name },
        ("dropdown" | "radio", Some(id)) => Spec::Choice {
            id,
            name,
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
            Spec::Toggle { id, name } | Spec::TriState { id, name } => Some((*id, name.as_str())),
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
            Spec::Toggle { id, name } => out.push(json!({
                "key": id.to_string(), "label": name, "kind": "toggle", "default": false,
            })),
            Spec::TriState { id, name } => out.push(json!({
                "key": id.to_string(), "label": name, "kind": "tags",
                "options": [{ "value": id.to_string(), "label": name }],
            })),
            Spec::Choice { id, name, choices } if !choices.is_empty() => out.push(json!({
                "key": id.to_string(),
                "label": name,
                "kind": "select",
                "options": choices
                    .iter()
                    .enumerate()
                    .map(|(index, label)| json!({ "value": index.to_string(), "label": label }))
                    .collect::<Vec<_>>(),
                "default": "0",
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

/// Every filter's default, by id: what Shosetsu hands a script before anything is chosen.
pub fn defaults(specs: &[Spec], data: &Table) -> mlua::Result<()> {
    for spec in specs {
        match spec {
            Spec::Text { id } => data.set(*id, "")?,
            Spec::Toggle { id, .. } => data.set(*id, false)?,
            Spec::TriState { id, .. } | Spec::Choice { id, .. } => data.set(*id, 0)?,
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
