//! What Raygun reads of an installed Kodi add-on: its settings (`resources/settings.xml`, Kodi's
//! old format and the `version="1"` one), shown as the extension's settings in Raygun; its
//! strings (`strings.po`, or the old `strings.xml`), which name them; and `kodi.json`, which
//! [`super::package`] writes beside the add-on's files.

use std::collections::BTreeMap;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde::{Deserialize, Serialize};

/// The file [`super::package`] writes into the extension's folder.
pub const PACKAGE_INFO: &str = "kodi.json";
/// Where the add-on's own files go in the extension's folder.
pub const ADDON_DIR: &str = "addon";
/// Where its dependencies go: `lib/<add-on id>/…`.
pub const LIBRARY_DIR: &str = "lib";

/// What kind of value a Kodi setting holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingKind {
    Bool,
    Number,
    Text,
    /// One of `options`, stored as the option's value (an index for Kodi's `enum`).
    Choice,
    /// Kodi keeps it but people don't set it (actions, folders, hidden values).
    Hidden,
}

/// One of an add-on's settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KodiSetting {
    pub id: String,
    pub kind: SettingKind,
    /// As shown, its string looked up.
    #[serde(default)]
    pub label: String,
    /// The value Kodi starts with, as Kodi stores it (`true`, `8`, `0`...).
    #[serde(default)]
    pub default: String,
    /// A choice's options: the stored value and its label.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<(String, String)>,
}

/// A dependency installed with the add-on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub id: String,
    pub name: String,
    pub version: String,
    /// Its `xbmc.python.module` folder, which goes on `sys.path` (`lib`, or `.`).
    #[serde(default)]
    pub library: String,
    /// Its settings' defaults, for `xbmcaddon.Addon('<id>').getSetting`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub settings: BTreeMap<String, String>,
}

/// `kodi.json`: the add-on and what was installed with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageInfo {
    /// Its Kodi id (`plugin.video.example`).
    pub addon: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub summary: String,
    /// Its script, relative to the add-on's folder (`default.py`).
    pub library: String,
    /// What it says it provides: `video`, `audio`.
    pub provides: Vec<String>,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub fanart: String,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub settings: Vec<KodiSetting>,
    /// Other add-ons it calls by address that Raygun answers itself (`plugin.video.youtube`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub answered: Vec<String>,
}

impl PackageInfo {
    pub fn provides_video(&self) -> bool {
        self.provides.iter().any(|kind| kind == "video")
    }

    pub fn provides_audio(&self) -> bool {
        self.provides.iter().any(|kind| kind == "audio")
    }
}

fn attribute(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref().eq_ignore_ascii_case(key))
        .and_then(|attribute| {
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.trim().to_owned())
        })
}

/// A label: a number is a string of the add-on's (or Kodi's), anything else is the text.
fn label_text(label: &str, strings: &BTreeMap<u32, String>) -> String {
    match label.trim().parse::<u32>() {
        Ok(number) => strings.get(&number).cloned().unwrap_or_default(),
        Err(_) => label.trim().to_owned(),
    }
}

/// A setting being read (the new format spreads one over several elements).
#[derive(Default)]
struct Pending {
    id: String,
    kind: String,
    label: String,
    default: Option<String>,
    options: Vec<(String, String)>,
    hidden: bool,
    level: u32,
    control: String,
}

impl Pending {
    fn finish(self, strings: &BTreeMap<u32, String>) -> Option<KodiSetting> {
        if self.id.is_empty() {
            return None;
        }
        let label = label_text(&self.label, strings);
        let kind = match self.kind.as_str() {
            _ if self.hidden || self.level >= 4 => SettingKind::Hidden,
            "bool" | "boolean" => SettingKind::Bool,
            "number" | "integer" | "slider" | "time" | "date" | "ipaddress"
                if self.options.is_empty() =>
            {
                if matches!(self.kind.as_str(), "time" | "date" | "ipaddress") {
                    SettingKind::Text
                } else {
                    SettingKind::Number
                }
            }
            "enum" | "labelenum" | "select" | "integer" | "string" | "number" => {
                if self.options.is_empty() {
                    SettingKind::Text
                } else {
                    SettingKind::Choice
                }
            }
            "text" if self.control != "button" => SettingKind::Text,
            _ => SettingKind::Hidden,
        };
        let options: Vec<(String, String)> = self
            .options
            .into_iter()
            .map(|(value, label)| {
                let shown = label_text(&label, strings);
                let shown = if shown.is_empty() {
                    value.clone()
                } else {
                    shown
                };
                (value, shown)
            })
            .collect();
        // An enum's default given as one of its labels stands for that option, as Kodi reads it.
        let mut default = self.default.unwrap_or_default();
        if kind == SettingKind::Choice
            && !options.iter().any(|(value, _)| *value == default)
            && let Some((value, _)) = options
                .iter()
                .find(|(value, shown)| *shown == default || label_text(value, strings) == default)
        {
            default = value.clone();
        }
        Some(KodiSetting {
            id: self.id,
            kind,
            label: if label.is_empty() {
                String::new()
            } else {
                label
            },
            default,
            options: if kind == SettingKind::Choice {
                options
            } else {
                Vec::new()
            },
        })
    }
}

/// The options of an old-format `enum` (`values="a|b"` or `lvalues="30001|30002"`, stored by
/// index) or `labelenum` (stored by label).
fn old_options(kind: &str, element: &BytesStart<'_>) -> Vec<(String, String)> {
    let values = attribute(element, "values");
    let lvalues = attribute(element, "lvalues");
    let labels: Vec<String> = match (&lvalues, &values) {
        (Some(lvalues), _) => lvalues.split('|').map(str::to_owned).collect(),
        (None, Some(values)) => values.split('|').map(str::to_owned).collect(),
        (None, None) => return Vec::new(),
    };
    labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            if kind == "labelenum" || kind == "select" {
                (label.clone(), label)
            } else {
                (index.to_string(), label)
            }
        })
        .collect()
}

/// An add-on's settings, in order, from its `resources/settings.xml`.
pub fn read_settings(xml: &str, strings: &BTreeMap<u32, String>) -> Vec<KodiSetting> {
    let mut reader = Reader::from_str(xml);
    let mut buffer = Vec::new();
    let mut found: Vec<KodiSetting> = Vec::new();
    let mut pending: Option<Pending> = None;
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut option_label = String::new();
    while let Ok(event) = reader.read_event_into(&mut buffer) {
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let name = element.name().as_ref().to_ascii_lowercase();
                match name.as_str() {
                    "setting" => {
                        if let Some(done) = pending.take().and_then(|done| done.finish(strings)) {
                            found.push(done);
                        }
                        let kind = attribute(&element, "type")
                            .unwrap_or_default()
                            .to_lowercase();
                        let mut setting = Pending {
                            id: attribute(&element, "id").unwrap_or_default(),
                            label: attribute(&element, "label").unwrap_or_default(),
                            default: attribute(&element, "default"),
                            hidden: attribute(&element, "visible")
                                .is_some_and(|visible| visible.eq_ignore_ascii_case("false")),
                            options: old_options(&kind, &element),
                            kind,
                            ..Pending::default()
                        };
                        if setting.kind == "action" || setting.kind.starts_with("list[") {
                            setting.hidden = true;
                        }
                        if empty {
                            if let Some(done) = setting.finish(strings) {
                                found.push(done);
                            }
                        } else {
                            pending = Some(setting);
                        }
                    }
                    "option" => {
                        option_label = attribute(&element, "label").unwrap_or_default();
                    }
                    "control" => {
                        if let Some(setting) = pending.as_mut() {
                            setting.control = attribute(&element, "type")
                                .unwrap_or_default()
                                .to_lowercase();
                        }
                    }
                    _ => {}
                }
                if !empty {
                    path.push(name);
                    text.clear();
                }
            }
            Event::Text(content) => text.push_str(&content),
            Event::GeneralRef(reference) => {
                let escaped = format!("&{};", &*reference);
                text.push_str(
                    &quick_xml::escape::unescape(&escaped)
                        .map(std::borrow::Cow::into_owned)
                        .unwrap_or(escaped),
                );
            }
            Event::End(_) => {
                let name = path.pop().unwrap_or_default();
                let value = std::mem::take(&mut text).trim().to_owned();
                match name.as_str() {
                    "setting" => {
                        if let Some(done) = pending.take().and_then(|done| done.finish(strings)) {
                            found.push(done);
                        }
                    }
                    "default" => {
                        if let Some(setting) = pending.as_mut() {
                            setting.default = Some(value);
                        }
                    }
                    "level" => {
                        if let Some(setting) = pending.as_mut() {
                            setting.level = value.parse().unwrap_or(0);
                        }
                    }
                    "visible" => {
                        if let Some(setting) = pending.as_mut()
                            && value.eq_ignore_ascii_case("false")
                        {
                            setting.hidden = true;
                        }
                    }
                    "option" => {
                        if let Some(setting) = pending.as_mut() {
                            let label = std::mem::take(&mut option_label);
                            let label = if label.is_empty() {
                                value.clone()
                            } else {
                                label
                            };
                            setting.options.push((value, label));
                        }
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if let Some(done) = pending.and_then(|done| done.finish(strings)) {
        found.push(done);
    }
    found
}

fn unquote_po(line: &str) -> String {
    let line = line.trim();
    let line = line
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(line);
    line.replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

/// Strings by number from a `strings.po` (`msgctxt "#30001"`), the translation where there is
/// one.
pub fn read_po(text: &str) -> BTreeMap<u32, String> {
    let mut table = BTreeMap::new();
    let mut number: Option<u32> = None;
    let mut field = "";
    let (mut id, mut translated) = (String::new(), String::new());
    let mut flush = |number: Option<u32>, id: &str, translated: &str| {
        if let Some(number) = number {
            let text = if translated.is_empty() {
                id
            } else {
                translated
            };
            table.insert(number, text.to_owned());
        }
    };
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("msgctxt") {
            flush(number, &id, &translated);
            id.clear();
            translated.clear();
            field = "";
            number = unquote_po(rest)
                .strip_prefix('#')
                .and_then(|digits| digits.parse().ok());
        } else if let Some(rest) = line.strip_prefix("msgid") {
            field = "id";
            id = unquote_po(rest);
        } else if let Some(rest) = line.strip_prefix("msgstr") {
            field = "str";
            translated = unquote_po(rest);
        } else if line.starts_with('"') {
            match field {
                "id" => id.push_str(&unquote_po(line)),
                "str" => translated.push_str(&unquote_po(line)),
                _ => {}
            }
        }
    }
    flush(number, &id, &translated);
    table
}

/// Strings by number from an old `strings.xml` (`<string id="30001">…</string>`).
pub fn read_strings_xml(text: &str) -> BTreeMap<u32, String> {
    let mut reader = Reader::from_str(text);
    let mut buffer = Vec::new();
    let mut table = BTreeMap::new();
    let mut current: Option<u32> = None;
    let mut value = String::new();
    while let Ok(event) = reader.read_event_into(&mut buffer) {
        match event {
            Event::Start(element) if element.name().as_ref() == "string" => {
                current = attribute(&element, "id").and_then(|id| id.parse().ok());
                value.clear();
            }
            Event::Text(content) => value.push_str(&content),
            Event::GeneralRef(reference) => {
                let escaped = format!("&{};", &*reference);
                value.push_str(
                    &quick_xml::escape::unescape(&escaped)
                        .map(std::borrow::Cow::into_owned)
                        .unwrap_or(escaped),
                );
            }
            Event::End(element) if element.name().as_ref() == "string" => {
                if let Some(number) = current.take() {
                    table.insert(number, value.trim().to_owned());
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    table
}

/// The settings people set in Raygun: Kodi's visible ones, as manifest settings.
pub fn manifest_settings(settings: &[KodiSetting]) -> Vec<serde_json::Value> {
    let mut seen = std::collections::HashSet::new();
    settings
        .iter()
        .filter(|setting| setting.kind != SettingKind::Hidden && seen.insert(&setting.id))
        .filter_map(|setting| {
            let label = if setting.label.is_empty() {
                setting.id.clone()
            } else {
                setting.label.clone()
            };
            let (kind, default) = match setting.kind {
                SettingKind::Bool => (
                    "boolean",
                    serde_json::Value::Bool(setting.default.eq_ignore_ascii_case("true")),
                ),
                SettingKind::Number => (
                    "number",
                    setting
                        .default
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                        .and_then(serde_json::Number::from_f64)
                        .map_or(serde_json::Value::Null, serde_json::Value::Number),
                ),
                SettingKind::Text => ("text", serde_json::Value::String(setting.default.clone())),
                SettingKind::Choice => {
                    let labels = choice_labels(setting);
                    if labels.is_empty() {
                        return None;
                    }
                    let shown = setting
                        .options
                        .iter()
                        .position(|(value, _)| *value == setting.default)
                        .and_then(|index| labels.get(index).cloned())
                        .unwrap_or_else(|| labels[0].clone());
                    ("select", serde_json::Value::String(shown))
                }
                SettingKind::Hidden => return None,
            };
            let mut def = serde_json::json!({ "key": setting.id, "type": kind, "label": label });
            if !default.is_null() {
                def["default"] = default;
            }
            if setting.kind == SettingKind::Choice {
                def["options"] = serde_json::json!(choice_labels(setting));
            }
            Some(def)
        })
        .collect()
}

/// A choice's labels as Raygun shows them, each once (a repeated label gets its value).
fn choice_labels(setting: &KodiSetting) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    setting
        .options
        .iter()
        .map(|(value, label)| {
            let text = if label.trim().is_empty() {
                value.clone()
            } else {
                label.clone()
            };
            if seen.insert(text.clone()) {
                text
            } else {
                format!("{text} ({value})")
            }
        })
        .collect()
}

/// A setting's value as Kodi stores it, from what's set in Raygun (`None` keeps the default).
pub fn kodi_value(setting: &KodiSetting, value: &serde_json::Value) -> Option<String> {
    match setting.kind {
        SettingKind::Bool => value
            .as_bool()
            .map(|on| if on { "true" } else { "false" }.to_owned()),
        SettingKind::Number => value.as_f64().map(|number| {
            if number.fract() == 0.0 && number.abs() < 1e15 {
                format!("{}", number as i64)
            } else {
                number.to_string()
            }
        }),
        SettingKind::Text => value.as_str().map(str::to_owned),
        SettingKind::Choice => {
            let chosen = value.as_str()?;
            let labels = choice_labels(setting);
            let index = labels.iter().position(|label| label == chosen)?;
            setting.options.get(index).map(|(value, _)| value.clone())
        }
        SettingKind::Hidden => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_become_raygun_settings() {
        let strings = read_po(
            "msgctxt \"#30000\"\nmsgid \"General\"\nmsgstr \"\"\n\nmsgctxt \"#30001\"\nmsgid \"Cache hours\"\nmsgstr \"\"\n\nmsgctxt \"#30002\"\nmsgid \"Quality\"\nmsgstr \"Qualität\"\n\nmsgctxt \"#30003\"\nmsgid \"High\"\nmsgstr \"\"\n\nmsgctxt \"#30004\"\nmsgid \"Low\"\nmsgstr \"\"\n",
        );
        assert_eq!(strings.get(&30002).map(String::as_str), Some("Qualität"));
        let xml = r#"<settings>
<category label="30000">
  <setting id="timeout" type="number" label="30001" default="8"/>
  <setting id="quality" type="enum" label="30002" lvalues="30003|30004" default="1"/>
  <setting id="source" type="labelenum" label="Source" values="Archive|Mirror" default="Mirror"/>
  <setting id="debug" type="bool" label="Debug" default="false"/>
  <setting id="token" type="text" label="Token" default="" visible="false"/>
  <setting id="clear" type="action" label="Clear" action="RunPlugin(plugin://x/?clear)"/>
  <setting type="sep"/>
</category></settings>"#;
        let settings = read_settings(xml, &strings);
        let kinds: Vec<(&str, SettingKind)> = settings
            .iter()
            .map(|setting| (setting.id.as_str(), setting.kind))
            .collect();
        assert_eq!(
            kinds,
            [
                ("timeout", SettingKind::Number),
                ("quality", SettingKind::Choice),
                ("source", SettingKind::Choice),
                ("debug", SettingKind::Bool),
                ("token", SettingKind::Hidden),
                ("clear", SettingKind::Hidden),
            ]
        );
        let shown = manifest_settings(&settings);
        assert_eq!(shown.len(), 4, "hidden values and actions aren't shown");
        assert_eq!(shown[0]["label"], "Cache hours");
        assert_eq!(shown[0]["default"], 8.0);
        assert_eq!(shown[1]["options"], serde_json::json!(["High", "Low"]));
        assert_eq!(
            shown[1]["default"], "Low",
            "Kodi's enum default is an index"
        );
        assert_eq!(shown[2]["default"], "Mirror", "a labelenum's is its label");
        let quality = &settings[1];
        assert_eq!(
            kodi_value(quality, &serde_json::json!("High")).as_deref(),
            Some("0")
        );
        assert_eq!(
            kodi_value(&settings[2], &serde_json::json!("Archive")).as_deref(),
            Some("Archive")
        );
        assert_eq!(
            kodi_value(&settings[3], &serde_json::json!(true)).as_deref(),
            Some("true")
        );
        assert_eq!(
            kodi_value(&settings[0], &serde_json::json!(12.0)).as_deref(),
            Some("12")
        );
        crate::manifest::Manifest::parse(
            &serde_json::json!({
                "id": "kodi.plugin.video.example", "name": "Example", "version": "1.0.0",
                "apiVersion": crate::manifest::API_VERSION, "runtime": "kodi",
                "entry": "addon/default.py", "provides": ["video.source"],
                "permissions": { "net": ["*"] }, "settings": shown,
            })
            .to_string(),
        )
        .expect("the settings make a valid manifest");
    }

    #[test]
    fn an_enums_default_may_be_its_label() {
        let xml = r#"<settings><setting id="language" type="enum" values="System default|English" default="System default"/></settings>"#;
        let settings = read_settings(xml, &BTreeMap::new());
        assert_eq!(settings[0].default, "0", "Kodi stores the option's index");
    }

    #[test]
    fn new_settings_read_their_parts() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<settings version="1"><section id="plugin.audio.example">
  <category id="stream" label="32000"><group id="1">
    <setting id="priority_quality" type="integer" label="32001">
      <level>0</level><default>2</default>
      <constraints><options>
        <option label="32910">0</option><option label="32911">1</option><option label="32912">2</option>
      </options></constraints>
      <control type="spinner" format="string"/>
    </setting>
    <setting id="username" type="string" label="Name"><level>0</level><default/><control type="edit" format="string"/></setting>
    <setting id="cache_days" type="integer" label="Days"><level>0</level><default>7</default><control type="slider" format="integer"/></setting>
    <setting id="internal" type="string"><level>4</level><default>x</default></setting>
    <setting id="clear" type="action" label="Clear"><data>RunScript(x)</data><control type="button" format="action"/></setting>
  </group></category>
</section></settings>"#;
        let strings = BTreeMap::from([
            (32001, "Quality".to_owned()),
            (32910, "Low".to_owned()),
            (32911, "Medium".to_owned()),
            (32912, "High".to_owned()),
        ]);
        let settings = read_settings(xml, &strings);
        assert_eq!(settings.len(), 5);
        assert_eq!(settings[0].kind, SettingKind::Choice);
        assert_eq!(settings[0].default, "2");
        assert_eq!(
            settings[0].options,
            [
                ("0".to_owned(), "Low".to_owned()),
                ("1".to_owned(), "Medium".to_owned()),
                ("2".to_owned(), "High".to_owned())
            ]
        );
        assert_eq!(settings[1].kind, SettingKind::Text);
        assert_eq!(settings[2].kind, SettingKind::Number);
        assert_eq!(settings[2].default, "7");
        assert_eq!(
            settings[3].kind,
            SettingKind::Hidden,
            "level 4 is Kodi's own"
        );
        assert_eq!(settings[4].kind, SettingKind::Hidden);
        let shown = manifest_settings(&settings);
        assert_eq!(shown[0]["default"], "High");
        assert_eq!(shown.len(), 3);
    }

    #[test]
    fn old_strings_files_read() {
        let table = read_strings_xml(
            r#"<?xml version="1.0"?><strings><string id="30001">Films &amp; more</string></strings>"#,
        );
        assert_eq!(table.get(&30001).map(String::as_str), Some("Films & more"));
    }
}
