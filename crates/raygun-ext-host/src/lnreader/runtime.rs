//! Running an LNReader plugin in Raygun's QuickJS runtime (`js.rs`) as LNReader runs it
//! (LNReader/lnreader, `src/plugins/pluginManager.ts`, MIT): the compiled CommonJS script inside a
//! function given `require` and `module`, in sloppy mode. The natives behind `prelude.js` keep
//! documents (`dom`), read addresses (`url`), record the plugin's settings and encode text in a
//! site's charset (`urlencode`).

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Function, Object};
use serde::Deserialize;
use serde_json::json;

use super::dom::Documents;
use crate::js::{err, eval_script, ok};
use crate::manifest::{SettingDef, SettingKind};

const PRELUDE: &str = include_str!("prelude.js");
/// Most settings one plugin may declare, and the longest key and label kept.
const MAX_SETTINGS: usize = 50;
const MAX_KEY_CHARS: usize = 100;
const MAX_LABEL_CHARS: usize = 80;
const MAX_OPTIONS: usize = 500;

/// A plugin ready to evaluate: its code inside LNReader's wrapper, then the start-up.
pub(crate) struct Prepared {
    code: String,
}

impl Prepared {
    pub fn new(code: &str) -> Result<Self, String> {
        if !code.contains("exports") {
            return Err("This isn't an LNReader plugin: it exports nothing".into());
        }
        Ok(Self {
            code: format!(
                "globalThis.__rg_ready = __lnrStart((function (require, module) {{\nconst exports = module.exports = {{}};\n{code}\n;return module.exports;\n}}).call(globalThis, __lnrRequire, {{}}));\n"
            ),
        })
    }
}

/// What the natives share with the rest of the extension.
pub(crate) struct Hooks {
    pub settings: Rc<RefCell<Vec<SettingDef>>>,
}

/// Installs the natives, evaluates the prelude, then the plugin and its start-up.
pub(crate) fn install(
    ctx: &Ctx<'_>,
    prepared: &Prepared,
    hooks: Hooks,
    filename: &str,
) -> Result<(), String> {
    install_web(ctx, hooks)?;
    eval_script(ctx, &prepared.code, filename, false)
        .map_err(|error| format!("The LNReader plugin stopped with an error: {error}"))
}

/// The natives and the prelude alone: the web a plugin finds (timers, base64, `URL`, `Headers`,
/// `fetch`, cheerio and the other libraries `__lnrRequire` hands out). The Nuvio runtime
/// (`crate::nuvio`) builds on them too.
pub(crate) fn install_web(ctx: &Ctx<'_>, hooks: Hooks) -> Result<(), String> {
    natives(ctx, hooks).map_err(|error| error.to_string())?;
    eval_script(ctx, PRELUDE, "lnreader:prelude", false)
}

fn envelope<T: serde::Serialize>(result: Result<T, String>) -> String {
    match result {
        Ok(value) => ok(value),
        Err(message) => err(message),
    }
}

fn natives(ctx: &Ctx<'_>, hooks: Hooks) -> rquickjs::Result<()> {
    let native = Object::new(ctx.clone())?;
    let documents = Rc::new(Documents::default());
    native.set(
        "dom",
        Function::new(ctx.clone(), move |op: String, args: String| {
            envelope(documents.run(&op, &args))
        })?,
    )?;
    native.set(
        "url",
        Function::new(ctx.clone(), |href: String, base: String| {
            envelope(parse_url(&href, &base))
        })?,
    )?;
    native.set(
        "encode",
        Function::new(ctx.clone(), |text: String, charset: String| {
            envelope(encode(&text, &charset))
        })?,
    )?;
    native.set(
        "decode",
        Function::new(ctx.clone(), |text: String, charset: String| {
            envelope(decode(&text, &charset))
        })?,
    )?;
    let settings = hooks.settings;
    native.set(
        "registerSetting",
        Function::new(ctx.clone(), move |setting: String| {
            envelope(register_setting(&settings, &setting))
        })?,
    )?;
    ctx.globals().set("__rgLnr", native)
}

/// An address and its parts, as the WHATWG `URL` gives them; `base` resolves a relative one.
fn parse_url(href: &str, base: &str) -> Result<serde_json::Value, String> {
    let url = if base.is_empty() {
        url::Url::parse(href)
    } else {
        url::Url::parse(base).and_then(|base| base.join(href))
    }
    .map_err(|error| format!("Invalid URL {href:?}: {error}"))?;
    let port = url.port().map(|port| port.to_string()).unwrap_or_default();
    let host = match (url.host_str(), url.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_owned(),
        (None, _) => String::new(),
    };
    Ok(json!({
        "href": url.as_str(),
        "protocol": format!("{}:", url.scheme()),
        "username": url.username(),
        "password": url.password().unwrap_or_default(),
        "host": host,
        "hostname": url.host_str().unwrap_or_default(),
        "port": port,
        "pathname": url.path(),
        "search": url.query().map(|query| format!("?{query}")).unwrap_or_default(),
        "hash": url.fragment().map(|fragment| format!("#{fragment}")).unwrap_or_default(),
        "origin": url.origin().ascii_serialization(),
    }))
}

fn charset(label: &str) -> Result<&'static encoding_rs::Encoding, String> {
    encoding_rs::Encoding::for_label(label.trim().as_bytes())
        .ok_or_else(|| format!("unknown charset {label:?}"))
}

/// `urlencode.encode`: percent-encoded bytes of `text` in `label`'s charset, as
/// `encodeURIComponent` leaves letters, digits and `-_.!~*'()` alone.
fn encode(text: &str, label: &str) -> Result<String, String> {
    let (bytes, _, _) = charset(label)?.encode(text);
    let mut encoded = String::with_capacity(bytes.len() * 3);
    for byte in bytes.iter() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(byte) {
            encoded.push(char::from(*byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    Ok(encoded)
}

/// `urlencode.decode`: percent-encoded bytes (and `+` as a space) read in `label`'s charset.
fn decode(text: &str, label: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut raw = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => match text
                .get(index + 1..index + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            {
                Some(byte) => {
                    raw.push(byte);
                    index += 3;
                }
                None => {
                    raw.push(b'%');
                    index += 1;
                }
            },
            b'+' => {
                raw.push(b' ');
                index += 1;
            }
            byte => {
                raw.push(byte);
                index += 1;
            }
        }
    }
    let (decoded, _, _) = charset(label)?.decode(&raw);
    Ok(decoded.into_owned())
}

/// A setting as the prelude sends it: LNReader's `pluginSettings` entry, with its key.
#[derive(Deserialize)]
struct RawSetting {
    key: String,
    #[serde(default)]
    label: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    value: serde_json::Value,
    /// A choice's options: `[label, value]` pairs, labels trimmed and unique.
    #[serde(default)]
    options: Vec<(String, String)>,
}

/// Records a setting the plugin declares, as Raygun shows settings: a switch for `Switch`, a
/// choice of labels for `Select`, text otherwise. Declaring a key again replaces it. (A Nuvio
/// provider's `onSettings` fields are declared the same way.)
pub(crate) fn register_setting(
    settings: &RefCell<Vec<SettingDef>>,
    json: &str,
) -> Result<(), String> {
    let raw: RawSetting =
        serde_json::from_str(json).map_err(|error| format!("invalid setting: {error}"))?;
    let key = raw.key.trim().to_owned();
    if key.is_empty() || key.chars().count() > MAX_KEY_CHARS {
        return Err("a setting needs a key of at most 100 characters".into());
    }
    let title: String = raw.label.trim().chars().take(MAX_LABEL_CHARS).collect();
    let label = (!title.is_empty()).then_some(title);
    let setting = match raw.kind.as_str() {
        "Switch" => SettingDef {
            key,
            kind: SettingKind::Boolean,
            label,
            options: Vec::new(),
            default: Some(json!(
                raw.value
                    .as_bool()
                    .unwrap_or_else(|| raw.value.as_str() == Some("true"))
            )),
            required: false,
        },
        "Select" if !raw.options.is_empty() => {
            if raw.options.len() > MAX_OPTIONS {
                return Err(format!("the setting {key} has too many options"));
            }
            let chosen = match &raw.value {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            let default = raw
                .options
                .iter()
                .find(|(_, value)| *value == chosen)
                .or_else(|| raw.options.first())
                .map(|(label, _)| json!(label));
            SettingDef {
                key,
                kind: SettingKind::Select,
                label,
                options: raw.options.into_iter().map(|(label, _)| label).collect(),
                default,
                required: false,
            }
        }
        _ => SettingDef {
            key,
            kind: SettingKind::Text,
            label,
            options: Vec::new(),
            default: Some(json!(match &raw.value {
                serde_json::Value::String(text) => text.clone(),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            })),
            required: false,
        },
    };
    let mut settings = settings.borrow_mut();
    if let Some(known) = settings.iter_mut().find(|known| known.key == setting.key) {
        *known = setting;
    } else if settings.len() < MAX_SETTINGS {
        settings.push(setting);
    } else {
        return Err("the plugin declared too many settings".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_the_plugin_as_lnreader_does() {
        let prepared = Prepared::new("exports.default = 1;").unwrap();
        assert!(
            prepared
                .code
                .contains("const exports = module.exports = {};")
        );
        assert!(prepared.code.contains("__lnrRequire"));
        assert!(Prepared::new("console.log(1)").is_err());
    }

    #[test]
    fn addresses_and_charsets() {
        let url = parse_url("/novel/1?x=y#top", "https://novels.example.org:8443/a/").unwrap();
        assert_eq!(
            url["href"],
            "https://novels.example.org:8443/novel/1?x=y#top"
        );
        assert_eq!(url["host"], "novels.example.org:8443");
        assert_eq!(url["search"], "?x=y");
        assert!(parse_url("not a url", "").is_err());
        assert_eq!(encode("中文 a", "gbk").unwrap(), "%D6%D0%CE%C4%20a");
        assert_eq!(decode("%D6%D0%CE%C4+a", "gbk").unwrap(), "中文 a");
        assert_eq!(encode("é", "utf-8").unwrap(), "%C3%A9");
    }

    #[test]
    fn plugin_settings_become_raygun_settings() {
        let settings = RefCell::new(Vec::new());
        let register = |json: serde_json::Value| register_setting(&settings, &json.to_string());
        register(
            json!({ "key": "enableVol", "label": "Volume view", "type": "Switch", "value": "" }),
        )
        .unwrap();
        register(
            json!({ "key": "mirror", "label": "Mirror", "type": "Select", "value": "b",
            "options": [["First", "a"], ["Second", "b"]] }),
        )
        .unwrap();
        register(json!({ "key": "token", "label": "Token", "type": "Text", "value": "" })).unwrap();
        let settings = settings.into_inner();
        assert_eq!(settings[0].kind, SettingKind::Boolean);
        assert_eq!(settings[0].default, Some(json!(false)));
        assert_eq!(settings[1].options, ["First", "Second"]);
        assert_eq!(settings[1].default, Some(json!("Second")));
        assert_eq!(settings[2].kind, SettingKind::Text);
    }
}
