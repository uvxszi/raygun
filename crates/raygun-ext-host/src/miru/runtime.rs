//! Running a Miru script in Raygun's QuickJS runtime (`js.rs`): the line Miru rewrites before
//! evaluating (`export default class …` becomes a named class), the natives behind Miru's
//! runtime API (HTML queries, XPath, settings, cookies), and the preludes evaluated first.
//! The script itself runs unchanged, in sloppy mode like Miru's `evaluate`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

use regex::Regex;
use rquickjs::{Ctx, Function, Object};
use serde::Deserialize;
use serde_json::json;

use super::dom::{Documents, Part, outer_html, text_content};
use super::header::{self, Header};
use super::xpath;
use crate::cookies::CookieJar;
use crate::js::{err, eval_script, ok};
use crate::manifest::{SettingDef, SettingKind};

const PRELUDE: &str = include_str!("prelude.js");
/// crypto-js (MIT), the build Miru bundles; evaluated only for scripts that use it.
const CRYPTO_JS: &str = include_str!("../../assets/crypto-js.min.js");
/// The name Miru's rewrite gives the script's class.
const CLASS: &str = "__MiruSource";
/// Most settings one script may register, and the longest key and label kept.
const MAX_SETTINGS: usize = 50;
const MAX_KEY_CHARS: usize = 100;
const MAX_LABEL_CHARS: usize = 80;
const MAX_OPTIONS: usize = 1000;

static EXPORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"export default class[^\r\n]*").expect("valid regex"));

/// A script ready to evaluate: its metadata and the code with Miru's rewrite and the start-up.
pub(crate) struct Prepared {
    pub header: Header,
    code: String,
    crypto: bool,
}

impl Prepared {
    pub fn new(code: &str) -> Result<Self, String> {
        let header = header::parse(code).ok_or(
            "This isn't a Miru extension: its ==MiruExtension== metadata is missing or incomplete",
        )?;
        if !EXPORT.is_match(code) {
            return Err(
                "This Miru extension defines no source (no `export default class`), so it can't run"
                    .into(),
            );
        }
        // As Miru does: the rest of the line becomes the class's opening.
        let opening = format!("class {CLASS} extends Extension {{");
        let rewritten = EXPORT.replace_all(code, regex::NoExpand(&opening));
        Ok(Self {
            crypto: code.contains("CryptoJS") || code.contains("md5"),
            code: format!("{rewritten}\n;globalThis.__rg_ready = __miruStart(new {CLASS}());\n"),
            header,
        })
    }
}

/// What the natives share with the rest of the extension.
pub(crate) struct Hooks {
    pub cookies: Arc<CookieJar>,
    pub settings: Rc<RefCell<Vec<SettingDef>>>,
}

/// Installs the natives, evaluates the preludes, then the script and its start-up.
pub(crate) fn install(
    ctx: &Ctx<'_>,
    prepared: &Prepared,
    hooks: Hooks,
    filename: &str,
) -> Result<(), String> {
    natives(ctx, &prepared.header, hooks).map_err(|error| error.to_string())?;
    eval_script(ctx, PRELUDE, "miru:prelude", false)?;
    if prepared.crypto {
        eval_script(ctx, CRYPTO_JS, "miru:crypto-js", false)?;
    }
    eval_script(ctx, &prepared.code, filename, false)
        .map_err(|error| format!("The Miru script stopped with an error: {error}"))
}

fn envelope<T: serde::Serialize>(result: Result<T, String>) -> String {
    match result {
        Ok(value) => ok(value),
        Err(message) => err(message),
    }
}

fn natives(ctx: &Ctx<'_>, header: &Header, hooks: Hooks) -> rquickjs::Result<()> {
    let native = Object::new(ctx.clone())?;
    let info = json!({
        "name": header.name,
        "package": header.package,
        "version": header.version,
        "type": header.kind.as_str(),
        "webSite": header.web_site,
    })
    .to_string();
    native.set("info", Function::new(ctx.clone(), move || info.clone())?)?;

    let documents = Rc::new(Documents::default());
    let d = documents.clone();
    native.set(
        "select",
        Function::new(
            ctx.clone(),
            move |content: String, selector: String, part: String| {
                envelope(d.query(&content, &selector, Part::parse(&part)))
            },
        )?,
    )?;
    let d = documents.clone();
    native.set(
        "selectAll",
        Function::new(ctx.clone(), move |content: String, selector: String| {
            envelope(d.query_all(&content, &selector))
        })?,
    )?;
    let d = documents.clone();
    native.set(
        "attr",
        Function::new(
            ctx.clone(),
            move |content: String, selector: String, name: String| {
                envelope(d.attribute(&content, &selector, &name))
            },
        )?,
    )?;
    let d = documents.clone();
    native.set(
        "remove",
        Function::new(ctx.clone(), move |content: String, selector: String| {
            envelope(d.remove(&content, &selector))
        })?,
    )?;
    let d = documents;
    native.set(
        "xpath",
        Function::new(
            ctx.clone(),
            move |content: String, expression: String, part: String| {
                envelope(query_xpath(&d, &content, &expression, &part))
            },
        )?,
    )?;

    let cookies = hooks.cookies;
    let jar = cookies.clone();
    native.set(
        "cookies",
        Function::new(ctx.clone(), move |url: String| {
            cookie_header(&cookies, &url)
        })?,
    )?;
    // The User-Agent for `url`'s site: the one a browser check there was passed with, which its
    // cookies hold only for, else the one sources send.
    native.set(
        "userAgent",
        Function::new(ctx.clone(), move |url: String| {
            reqwest::Url::parse(&url)
                .ok()
                .and_then(|url| jar.user_agent_for(&url))
                .unwrap_or_else(raygun_net::user_agent)
        })?,
    )?;
    let settings = hooks.settings;
    native.set(
        "registerSetting",
        Function::new(ctx.clone(), move |setting: String| {
            envelope(register_setting(&settings, &setting))
        })?,
    )?;
    ctx.globals().set("__rgMiru", native)
}

/// Miru's `queryXPath` answers (Dart's `xpath_selector`): the first node's text or outer HTML,
/// the first attribute, the attributes as a JSON string, or every node's outer HTML.
fn query_xpath(
    documents: &Documents,
    content: &str,
    expression: &str,
    part: &str,
) -> Result<serde_json::Value, String> {
    let html = documents.parse(content);
    let items = xpath::evaluate(&html, expression)?;
    let attributes = xpath::attributes(&html, &items);
    // `count(…)` and other values that aren't nodes.
    let value_only =
        matches!(items.as_slice(), [xpath::Item::Attr { name, .. }] if name.is_empty());
    let nodes: Vec<_> = if value_only {
        Vec::new()
    } else {
        xpath::nodes(&items)
            .into_iter()
            .filter_map(|id| html.tree.get(id))
            .collect()
    };
    Ok(match part {
        "attr" => json!(attributes.first().cloned().unwrap_or_default()),
        "attrs" => json!(serde_json::to_string(&attributes).unwrap_or_default()),
        "allHTML" => json!(
            nodes
                .iter()
                .map(|node| outer_html(*node))
                .collect::<Vec<_>>()
        ),
        "outerHTML" => json!(
            nodes
                .first()
                .map(|node| outer_html(*node))
                .unwrap_or_default()
        ),
        _ if value_only => json!(attributes.first().cloned().unwrap_or_default()),
        _ => json!(
            nodes
                .first()
                .map(|node| text_content(*node))
                .unwrap_or_default()
        ),
    })
}

/// The cookies the extension's jar would send to `url`, as a `Cookie` header.
fn cookie_header(jar: &CookieJar, url: &str) -> String {
    use reqwest::cookie::CookieStore;
    reqwest::Url::parse(url)
        .ok()
        .and_then(|url| jar.cookies(&url))
        .and_then(|value| value.to_str().ok().map(str::to_owned))
        .unwrap_or_default()
}

/// A setting as the prelude sends it: Miru's fields, radio options as `[label, value]` pairs
/// (labels already trimmed and unique).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSetting {
    key: String,
    title: String,
    #[serde(rename = "type")]
    kind: String,
    default_value: String,
    #[serde(default)]
    options: Vec<(String, String)>,
}

/// Records a setting the script registered, as Raygun shows settings: text for `input`, a
/// switch for `toggle`, a choice of labels for `radio`. Registering a key again replaces it.
fn register_setting(settings: &RefCell<Vec<SettingDef>>, json: &str) -> Result<(), String> {
    let raw: RawSetting =
        serde_json::from_str(json).map_err(|error| format!("invalid setting: {error}"))?;
    let key = raw.key.trim().to_owned();
    if key.is_empty() || key.chars().count() > MAX_KEY_CHARS {
        return Err("a setting needs a key of at most 100 characters".into());
    }
    let title: String = raw.title.trim().chars().take(MAX_LABEL_CHARS).collect();
    let label = (!title.is_empty()).then_some(title);
    let setting = match raw.kind.as_str() {
        "toggle" => SettingDef {
            key,
            kind: SettingKind::Boolean,
            label,
            options: Vec::new(),
            default: Some(json!(raw.default_value == "true")),
        },
        "radio" if !raw.options.is_empty() => {
            if raw.options.len() > MAX_OPTIONS {
                return Err(format!("the setting {key} has too many options"));
            }
            let default = raw
                .options
                .iter()
                .find(|(_, value)| *value == raw.default_value)
                .map(|(label, _)| json!(label));
            SettingDef {
                key,
                kind: SettingKind::Select,
                label,
                options: raw.options.into_iter().map(|(label, _)| label).collect(),
                default,
            }
        }
        _ => SettingDef {
            key,
            kind: SettingKind::Text,
            label,
            options: Vec::new(),
            default: Some(json!(raw.default_value)),
        },
    };
    let mut settings = settings.borrow_mut();
    if let Some(known) = settings.iter_mut().find(|known| known.key == setting.key) {
        *known = setting;
    } else if settings.len() < MAX_SETTINGS {
        settings.push(setting);
    } else {
        return Err("the extension registered too many settings".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_the_export_line_like_miru() {
        let code = "// ==MiruExtension==\n// @name X\n// @version v1\n// @package x.y\n// @type manga\n// @webSite https://x.example\n// ==/MiruExtension==\nexport default class extends Extension { async latest() { return []; } }\n";
        let prepared = Prepared::new(code).unwrap();
        assert!(
            prepared
                .code
                .contains("class __MiruSource extends Extension {\n")
        );
        assert!(
            !prepared.code.contains("async latest"),
            "the rest of the line goes, as in Miru"
        );
        assert!(!prepared.crypto);
        assert!(Prepared::new("// @name X\n").is_err());
        let no_class = code.replace("export default class", "class");
        assert!(
            Prepared::new(&no_class)
                .err()
                .unwrap()
                .contains("defines no source")
        );
    }

    #[test]
    fn settings_become_raygun_settings() {
        let settings = RefCell::new(Vec::new());
        let register = |json: serde_json::Value| register_setting(&settings, &json.to_string());
        register(json!({ "key": "domain", "title": " Site ", "type": "input", "defaultValue": "https://a.example" })).unwrap();
        register(json!({ "key": "hd", "title": "HD", "type": "toggle", "defaultValue": "true" }))
            .unwrap();
        register(
            json!({ "key": "expire", "title": "Cache", "type": "radio", "defaultValue": "60",
            "options": [["none", "0"], ["1 hour", "60"]] }),
        )
        .unwrap();
        register(json!({ "key": "domain", "title": "Domain", "type": "input", "defaultValue": "https://b.example" })).unwrap();
        assert!(
            register(json!({ "key": " ", "title": "", "type": "input", "defaultValue": "" }))
                .is_err()
        );
        let settings = settings.into_inner();
        assert_eq!(settings.len(), 3, "a key registered again is replaced");
        assert_eq!(settings[0].label.as_deref(), Some("Domain"));
        assert_eq!(settings[0].default, Some(json!("https://b.example")));
        assert_eq!(settings[1].kind, SettingKind::Boolean);
        assert_eq!(settings[1].default, Some(json!(true)));
        assert_eq!(settings[2].kind, SettingKind::Select);
        assert_eq!(settings[2].options, ["none", "1 hour"]);
        assert_eq!(
            settings[2].default,
            Some(json!("1 hour")),
            "the default value's label"
        );
    }
}
