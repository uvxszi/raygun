//! Site scripts (`site.scripts`): scripts and styles that Raygun's websites browser adds to the
//! pages of the sites an extension names, from its manifest's `site` section
//! (`docs/extensions/site-scripts.md`).
//!
//! Nothing of them runs in Raygun. The browser (Android's `WebsiteActivity` in its own process,
//! desktop site windows with their own profile and no IPC) injects one bundle per extension,
//! composed here: the extension's files wrapped in a function that checks the page's address
//! against the extension's match patterns, adds its CSS and hands its scripts `raygunPage`
//! (their settings, the browser's state, and offers, which are data Raygun checks before acting
//! on them). They run in the page with the page's privileges only.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde::Deserialize;

/// The provider kind a site-scripts extension provides.
pub const KIND: &str = "site.scripts";
const MAX_GROUPS: usize = 16;
const MAX_PATTERNS: usize = 32;
const MAX_FILES: usize = 16;
/// The longest pattern read.
const MAX_PATTERN_CHARS: usize = 512;

/// The manifest's `site` section as written.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RawSite {
    scripts: Vec<RawGroup>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawGroup {
    matches: Vec<String>,
    #[serde(default)]
    exclude_matches: Vec<String>,
    #[serde(default)]
    js: Vec<String>,
    #[serde(default)]
    css: Vec<String>,
    #[serde(default)]
    run_at: RunAt,
    #[serde(default)]
    all_frames: bool,
}

/// When a group's scripts run in a page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunAt {
    /// Before the page's own scripts (its CSS is added then too).
    DocumentStart,
    /// Once the document is parsed (`DOMContentLoaded`).
    DocumentEnd,
    /// Once the page has loaded.
    #[default]
    DocumentIdle,
}

impl RunAt {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DocumentStart => "document-start",
            Self::DocumentEnd => "document-end",
            Self::DocumentIdle => "document-idle",
        }
    }
}

/// A validated `site` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteScripts {
    pub groups: Vec<ScriptGroup>,
}

/// Scripts and styles for the pages some patterns match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptGroup {
    pub matches: Vec<MatchPattern>,
    pub exclude_matches: Vec<MatchPattern>,
    pub js: Vec<String>,
    pub css: Vec<String>,
    pub run_at: RunAt,
    /// Frames too, not only the top page.
    pub all_frames: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Scheme {
    Http,
    Https,
    /// `*`: http and https.
    Web,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Host {
    /// `*`: every host.
    Any,
    /// `*.example.com`: the domain and every subdomain.
    Domain(String),
    /// `example.com`.
    Exact(String),
}

/// A match pattern (a subset of WebExtensions'): `<scheme>://<host><path>`, where the scheme is
/// `https`, `http` or `*` (both), the host `*`, `*.example.com` (the domain and its subdomains)
/// or `example.com`, and the path starts with `/`, `*` matching any characters. A page's port
/// and fragment don't count; its query does, as part of the path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchPattern {
    source: String,
    scheme: Scheme,
    host: Host,
    path: String,
}

impl std::fmt::Display for MatchPattern {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.source)
    }
}

/// Which browser a bundle is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Android,
    Desktop,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Android => "android",
            Self::Desktop => "desktop",
        }
    }
}

impl MatchPattern {
    pub fn parse(pattern: &str) -> Option<Self> {
        if pattern.is_empty() || pattern.chars().count() > MAX_PATTERN_CHARS {
            return None;
        }
        let (scheme, rest) = pattern.split_once("://")?;
        let scheme = match scheme {
            "https" => Scheme::Https,
            "http" => Scheme::Http,
            "*" => Scheme::Web,
            _ => return None,
        };
        let slash = rest.find('/')?;
        let (host, path) = rest.split_at(slash);
        let host = if host == "*" {
            Host::Any
        } else if let Some(domain) = host.strip_prefix("*.") {
            is_host(domain).then(|| Host::Domain(domain.to_owned()))?
        } else {
            is_host(host).then(|| Host::Exact(host.to_owned()))?
        };
        if path.chars().any(|char| char.is_whitespace() || char == '#') {
            return None;
        }
        Some(Self {
            source: pattern.to_owned(),
            scheme,
            host,
            path: path.to_owned(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// Whether the pattern matches `url`.
    pub fn matches(&self, url: &url::Url) -> bool {
        let scheme = match url.scheme() {
            "https" => Scheme::Https,
            "http" => Scheme::Http,
            _ => return false,
        };
        if self.scheme != Scheme::Web && self.scheme != scheme {
            return false;
        }
        let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
        let host_fits = match &self.host {
            Host::Any => !host.is_empty(),
            Host::Domain(domain) => {
                host == *domain
                    || host
                        .strip_suffix(domain.as_str())
                        .is_some_and(|start| start.ends_with('.'))
            }
            Host::Exact(exact) => host == *exact,
        };
        if !host_fits {
            return false;
        }
        let mut path = url.path().to_owned();
        if let Some(query) = url.query() {
            path.push('?');
            path.push_str(query);
        }
        glob(&self.path, &path)
    }

    /// The pattern as a JavaScript regular expression for a page's address without its fragment,
    /// read the way [`Self::matches`] reads it.
    pub fn js_regex(&self) -> String {
        let scheme = match self.scheme {
            Scheme::Https => "https",
            Scheme::Http => "http",
            Scheme::Web => "https?",
        };
        let host = match &self.host {
            Host::Any => "[^/:?#@]+".to_owned(),
            Host::Domain(domain) => format!("(?:[^/:?#@]+\\.)?{}", escape(domain)),
            Host::Exact(exact) => escape(exact),
        };
        let path = self
            .path
            .split('*')
            .map(escape)
            .collect::<Vec<_>>()
            .join(".*");
        format!("^{scheme}://(?:[^/?#@]*@)?{host}(?::\\d+)?{path}$")
    }

    /// The origins a browser may inject into for this pattern, as androidx.webkit's
    /// `allowedOriginRules` take them (`https://example.com`, `https://*.example.com`, or `*`).
    /// The bundle checks the rest of the pattern itself.
    pub fn origin_rules(&self) -> Vec<String> {
        let schemes: &[&str] = match self.scheme {
            Scheme::Https => &["https"],
            Scheme::Http => &["http"],
            Scheme::Web => &["https", "http"],
        };
        match &self.host {
            Host::Any => vec!["*".to_owned()],
            Host::Domain(domain) => schemes
                .iter()
                .flat_map(|scheme| {
                    [
                        format!("{scheme}://{domain}"),
                        format!("{scheme}://*.{domain}"),
                    ]
                })
                .collect(),
            Host::Exact(exact) => schemes
                .iter()
                .map(|scheme| format!("{scheme}://{exact}"))
                .collect(),
        }
    }

    /// The site it names, for people: `youtube.com`, or `*` for every site.
    pub fn site_label(&self) -> String {
        match &self.host {
            Host::Any => "*".to_owned(),
            Host::Domain(domain) => domain.clone(),
            Host::Exact(exact) => exact.strip_prefix("www.").unwrap_or(exact).to_owned(),
        }
    }
}

/// A host as patterns name it: lowercase labels of letters, digits and `-`, at least two.
fn is_host(host: &str) -> bool {
    host.len() <= 253
        && host.split('.').count() >= 2
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

/// `text` against `pattern`, where `*` matches any characters (`/` too).
fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let [first, middle @ .., last] = parts.as_slice() else {
        return pattern == text;
    };
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

/// Text as a JavaScript regular expression's literal characters.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for char in text.chars() {
        if "\\^$.|?*+()[]{}/-".contains(char) {
            escaped.push('\\');
        }
        escaped.push(char);
    }
    escaped
}

/// A path to one of the extension's files: relative, `/`-separated, without `..`.
fn is_safe_path(path: &str, extension: &str) -> bool {
    path.ends_with(extension)
        && path.len() <= 200
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

impl SiteScripts {
    pub(crate) fn parse(raw: RawSite) -> Result<Self, String> {
        if raw.scripts.is_empty() || raw.scripts.len() > MAX_GROUPS {
            return Err(format!("site.scripts needs 1 to {MAX_GROUPS} groups"));
        }
        let mut groups = Vec::new();
        for (index, group) in raw.scripts.into_iter().enumerate() {
            let at = format!("site.scripts[{index}]");
            let patterns = |list: Vec<String>, name: &str, at_least: usize| {
                if list.len() < at_least || list.len() > MAX_PATTERNS {
                    return Err(format!(
                        "{at}.{name} takes {at_least} to {MAX_PATTERNS} patterns"
                    ));
                }
                list.iter()
                    .map(|pattern| {
                        MatchPattern::parse(pattern).ok_or_else(|| {
                            format!(
                                "{at}.{name}: {pattern:?} isn't a match pattern like https://*.example.com/*"
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            };
            let matches = patterns(group.matches, "matches", 1)?;
            let exclude_matches = patterns(group.exclude_matches, "excludeMatches", 0)?;
            for (list, name, kind) in [(&group.js, "js", ".js"), (&group.css, "css", ".css")] {
                if list.len() > MAX_FILES {
                    return Err(format!("{at}.{name} takes at most {MAX_FILES} files"));
                }
                if let Some(path) = list.iter().find(|path| !is_safe_path(path, kind)) {
                    return Err(format!(
                        "{at}.{name}: {path:?} must be a relative path to a {kind} file inside the extension"
                    ));
                }
            }
            if group.js.is_empty() && group.css.is_empty() {
                return Err(format!("{at} has no js or css files"));
            }
            groups.push(ScriptGroup {
                matches,
                exclude_matches,
                js: group.js,
                css: group.css,
                run_at: group.run_at,
                all_frames: group.all_frames,
            });
        }
        Ok(Self { groups })
    }

    /// Every pattern the scripts run on, sorted, once each: what repositories list as `sites`
    /// and what people are told before installing.
    pub fn patterns(&self) -> Vec<String> {
        self.groups
            .iter()
            .flat_map(|group| &group.matches)
            .map(|pattern| pattern.as_str().to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Every file the scripts use, once each.
    pub fn files(&self) -> Vec<&str> {
        let mut seen = BTreeSet::new();
        self.groups
            .iter()
            .flat_map(|group| group.js.iter().chain(&group.css))
            .map(String::as_str)
            .filter(|path| seen.insert(*path))
            .collect()
    }

    /// Whether a group's scripts run on the page at `url` (the top page).
    pub fn matches(&self, url: &url::Url) -> bool {
        self.groups.iter().any(|group| group.runs_on(url))
    }

    /// The origins a browser injects the bundle into.
    pub fn origin_rules(&self) -> Vec<String> {
        origin_rules(self.groups.iter().flat_map(|group| &group.matches))
    }
}

impl ScriptGroup {
    /// Whether it runs on the top page at `url`: a pattern matches it and no exclusion does.
    pub fn runs_on(&self, url: &url::Url) -> bool {
        self.matches.iter().any(|pattern| pattern.matches(url))
            && !self
                .exclude_matches
                .iter()
                .any(|pattern| pattern.matches(url))
    }
}

/// The origin rules of some patterns, once each; `*` alone when one is every site.
pub fn origin_rules<'a>(patterns: impl IntoIterator<Item = &'a MatchPattern>) -> Vec<String> {
    let rules: BTreeSet<String> = patterns
        .into_iter()
        .flat_map(MatchPattern::origin_rules)
        .collect();
    if rules.contains("*") {
        return vec!["*".to_owned()];
    }
    rules.into_iter().collect()
}

/// Whether any of `patterns` (as repositories list them) matches `url`; unreadable ones don't.
pub fn any_matches(patterns: &[String], url: &url::Url) -> bool {
    patterns
        .iter()
        .filter_map(|pattern| MatchPattern::parse(pattern))
        .any(|pattern| pattern.matches(url))
}

/// One extension's scripts and styles as the one script the browser injects: the files of every
/// group, in the order the manifest lists them, behind the checks of its patterns, frames and
/// timing. `read` gives a file's text; `settings` are the values the scripts see.
pub fn compose(
    id: &str,
    version: &str,
    scripts: &SiteScripts,
    read: impl Fn(&str) -> Result<String, String>,
    settings: &serde_json::Map<String, serde_json::Value>,
    platform: Platform,
) -> Result<String, String> {
    let mut code = String::new();
    let _ = writeln!(
        code,
        "// {id} {version}: site scripts composed by Raygun for its websites browser."
    );
    code.push_str("(function () {\n");
    let _ = writeln!(code, "var ID = {};", js_string(id));
    let _ = writeln!(code, "var VERSION = {};", js_string(version));
    let _ = writeln!(code, "var PLATFORM = {};", js_string(platform.as_str()));
    let _ = writeln!(
        code,
        "var SETTINGS = {};",
        serde_json::Value::Object(settings.clone())
    );
    code.push_str(RUNTIME);
    code.push_str("var groups = [\n");
    for group in &scripts.groups {
        let regexes = |patterns: &[MatchPattern]| {
            serde_json::Value::Array(
                patterns
                    .iter()
                    .map(|pattern| serde_json::Value::String(pattern.js_regex()))
                    .collect(),
            )
        };
        let mut css = String::new();
        for path in &group.css {
            css.push_str(&read(path)?);
            css.push('\n');
        }
        let _ = writeln!(
            code,
            "{{ matches: {}, excludes: {}, allFrames: {}, runAt: {}, css: {}, scripts: [",
            regexes(&group.matches),
            regexes(&group.exclude_matches),
            group.all_frames,
            js_string(group.run_at.as_str()),
            js_string(&css),
        );
        for path in &group.js {
            let text = read(path)?;
            let _ = writeln!(code, "// {path}");
            code.push_str("function (raygunPage) {\n");
            code.push_str(&text);
            code.push_str("\n},\n");
        }
        code.push_str("] },\n");
    }
    code.push_str("];\nstart(groups);\n})();\n");
    Ok(code)
}

/// Text as a JavaScript string literal (JSON's is one).
fn js_string(text: &str) -> String {
    serde_json::Value::String(text.to_owned()).to_string()
}

/// What every bundle runs: the checks, `raygunPage`, styles and timing. The bridge Raygun's
/// browser injects first (`page-bridge.js`) keeps the browser's state under
/// `Symbol.for('raygun.page')`; without it (an old WebView) scripts still run, without offers.
const RUNTIME: &str = r#"var HUB = Symbol.for('raygun.page');
var DONE = Symbol.for('raygun.site.done');
var done = window[DONE] || {};
if (!window[DONE]) {
  try { Object.defineProperty(window, DONE, { value: done }); } catch (error) { return; }
}
if (done[ID]) return;
done[ID] = true;
var top = false;
try { top = window === window.top; } catch (error) { top = false; }
var address = String(location.href).split('#')[0];
var IDLE_STATE = Object.freeze({ background: false, call: false, audioActive: false });
function hub() { return window[HUB]; }
function post(message) {
  var bridge = hub();
  if (bridge && typeof bridge.post === 'function') { bridge.post(message); return; }
  try {
    if (window.raygunBridge && typeof window.raygunBridge.postMessage === 'function') {
      window.raygunBridge.postMessage(JSON.stringify(message));
    } else if (window.chrome && window.chrome.webview && top) {
      window.chrome.webview.postMessage({ raygunBridge: message });
    }
  } catch (error) { /* the browser isn't listening */ }
}
function text(value, limit) {
  return typeof value === 'string' && value.trim() ? value.trim().slice(0, limit) : undefined;
}
function web(value) {
  return typeof value === 'string' && /^https?:\/\//i.test(value) && value.length <= 2048 ? value : undefined;
}
var KINDS = ['video', 'song', 'playlist'];
var api = Object.freeze({
  extension: ID,
  version: VERSION,
  platform: PLATFORM,
  settings: Object.freeze(SETTINGS),
  get state() { var bridge = hub(); return bridge && bridge.state ? bridge.state : IDLE_STATE; },
  offer: function (offer) {
    if (!offer || KINDS.indexOf(offer.kind) < 0 || !web(offer.url)) return;
    var duration = typeof offer.durationMs === 'number' && isFinite(offer.durationMs) && offer.durationMs > 0
      ? Math.round(offer.durationMs) : undefined;
    post({ type: 'offer', extension: ID, page: String(location.href), offer: {
      kind: offer.kind, url: offer.url, title: text(offer.title, 300), artist: text(offer.artist, 300),
      album: text(offer.album, 300), artworkUrl: web(offer.artworkUrl), durationMs: duration } });
  },
  withdraw: function (url) {
    post({ type: 'withdraw', extension: ID, page: String(location.href), url: typeof url === 'string' ? url : null });
  },
  on: function (event, listener) {
    var bridge = hub();
    if (event !== 'state' || typeof listener !== 'function' || !bridge || typeof bridge.on !== 'function') {
      return function () {};
    }
    return bridge.on(listener);
  }
});
function fits(group) {
  if (!group.allFrames && !top) return false;
  var hit = function (source) { return new RegExp(source).test(address); };
  return group.matches.some(hit) && !group.excludes.some(hit);
}
function addStyle(css) {
  if (!css) return;
  var sheet = null;
  try {
    if (typeof CSSStyleSheet === 'function' && 'adoptedStyleSheets' in document) {
      sheet = new CSSStyleSheet();
      sheet.replaceSync(css);
    }
  } catch (error) { sheet = null; }
  if (sheet) {
    var adopt = function () {
      try {
        if (document.adoptedStyleSheets.indexOf(sheet) < 0) {
          document.adoptedStyleSheets = document.adoptedStyleSheets.concat([sheet]);
        }
      } catch (error) { /* left as it is */ }
    };
    adopt();
    // Pages that set the list themselves drop it: it's added again once they've loaded.
    document.addEventListener('DOMContentLoaded', adopt);
    window.addEventListener('load', adopt);
    return;
  }
  var style = document.createElement('style');
  style.textContent = css;
  var place = function () {
    var parent = document.head || document.documentElement;
    if (!parent) return false;
    parent.appendChild(style);
    return true;
  };
  if (!place()) {
    var watcher = new MutationObserver(function () { if (place()) watcher.disconnect(); });
    watcher.observe(document, { childList: true, subtree: true });
  }
}
function when(runAt, go) {
  if (runAt === 'document-start') { go(); return; }
  if (runAt === 'document-end') {
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', go, { once: true });
    else go();
    return;
  }
  if (document.readyState === 'complete') setTimeout(go, 0);
  else window.addEventListener('load', function () { setTimeout(go, 0); }, { once: true });
}
function start(list) {
  list.forEach(function (group) {
    if (!fits(group)) return;
    addStyle(group.css);
    when(group.runAt, function () {
      group.scripts.forEach(function (script) {
        try { script.call(window, api); } catch (error) { console.error('[' + ID + ']', error); }
      });
    });
  });
}
"#;

/// The provider a site-scripts extension shows as (it's never called).
pub fn provider_id(extension_id: &str) -> String {
    format!("{extension_id}/scripts")
}

/// A site-scripts extension as loaded: its manifest and the user's settings.
pub(crate) struct LoadedSite {
    pub manifest: crate::Manifest,
    pub user_settings: std::collections::BTreeMap<String, serde_json::Value>,
}

impl LoadedSite {
    pub fn providers(&self) -> Vec<raygun_model::ProviderInfo> {
        vec![raygun_model::ProviderInfo {
            id: provider_id(&self.manifest.id),
            extension_id: self.manifest.id.clone(),
            kind: KIND.to_owned(),
            name: self.manifest.name.clone(),
            search_pill: None,
            adult: false,
        }]
    }
}

/// A site-scripts extension that loaded and is on, as the websites browser needs it.
#[derive(Debug, Clone)]
pub struct SiteExtension {
    pub id: String,
    pub name: String,
    pub version: String,
    pub scripts: SiteScripts,
    /// What its scripts see as `raygunPage.settings`: the user's values, else the defaults.
    pub settings: serde_json::Map<String, serde_json::Value>,
    source: crate::ExtensionSource,
}

impl SiteExtension {
    pub(crate) fn new(
        manifest: &crate::Manifest,
        scripts: SiteScripts,
        settings: serde_json::Map<String, serde_json::Value>,
        source: crate::ExtensionSource,
    ) -> Self {
        Self {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.to_string(),
            scripts,
            settings,
            source,
        }
    }

    /// The one script the browser injects for it ([`compose`]).
    pub fn compose(&self, platform: Platform) -> Result<String, String> {
        compose(
            &self.id,
            &self.version,
            &self.scripts,
            |path| self.source.read(path),
            &self.settings,
            platform,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> url::Url {
        url::Url::parse(text).unwrap()
    }

    fn pattern(text: &str) -> MatchPattern {
        MatchPattern::parse(text).unwrap_or_else(|| panic!("{text} should parse"))
    }

    #[test]
    fn patterns_parse_as_webextensions_write_them() {
        for good in [
            "https://www.youtube.com/*",
            "*://*.youtube.com/*",
            "http://example.com/watch?v=*",
            "*://*/*",
            "https://example.co.uk/a/*/b",
        ] {
            assert!(MatchPattern::parse(good).is_some(), "{good}");
        }
        for bad in [
            "",
            "<all_urls>",
            "ftp://example.com/*",
            "https://example.com",
            "https://Example.com/*",
            "https://*example.com/*",
            "https://example.com:8080/*",
            "https://localhost/*",
            "https://exa_mple.com/*",
            "https://example.com/#x",
            "file:///tmp/*",
            "javascript://x.com/*",
        ] {
            assert!(MatchPattern::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn patterns_match_scheme_host_path_and_query() {
        let youtube = pattern("https://*.youtube.com/*");
        assert!(youtube.matches(&url("https://youtube.com/")));
        assert!(youtube.matches(&url("https://m.youtube.com/watch?v=abc")));
        assert!(youtube.matches(&url("https://music.youtube.com:443/x#frag")));
        assert!(!youtube.matches(&url("http://m.youtube.com/")));
        assert!(!youtube.matches(&url("https://notyoutube.com/")));
        assert!(!youtube.matches(&url("https://youtube.com.evil.example/")));

        let watch = pattern("*://www.example.com/watch?v=*");
        assert!(watch.matches(&url("http://www.example.com/watch?v=1")));
        assert!(watch.matches(&url("https://www.example.com:8443/watch?v=1&t=2")));
        assert!(!watch.matches(&url("https://www.example.com/watch")));
        assert!(!watch.matches(&url("https://example.com/watch?v=1")));

        let any = pattern("*://*/*");
        assert!(any.matches(&url("https://a.b/")));
        assert!(!any.matches(&url("file:///etc/hosts")));

        let middle = pattern("https://a.example/x/*/y");
        assert!(middle.matches(&url("https://a.example/x/1/2/y")));
        assert!(!middle.matches(&url("https://a.example/x/1/y/z")));
    }

    #[test]
    fn javascript_regexes_read_addresses_as_the_rust_matcher_does() {
        let cases = [
            ("https://*.youtube.com/*", "https://youtube.com/"),
            (
                "https://*.youtube.com/*",
                "https://m.youtube.com/watch?v=abc",
            ),
            (
                "https://*.youtube.com/*",
                "https://music.youtube.com:8443/x",
            ),
            ("https://*.youtube.com/*", "http://m.youtube.com/"),
            (
                "https://*.youtube.com/*",
                "https://youtube.com.evil.example/",
            ),
            (
                "https://*.youtube.com/*",
                "https://evil.example/https://m.youtube.com/",
            ),
            (
                "*://www.example.com/watch?v=*",
                "http://www.example.com/watch?v=1",
            ),
            (
                "*://www.example.com/watch?v=*",
                "https://www.example.com/watchxv=1",
            ),
            ("*://*/*", "https://a.b/"),
            ("https://a.example/x/*/y", "https://a.example/x/1/2/y"),
            ("https://a.example/x/*/y", "https://a.example/x/1/y/z"),
            ("https://a.example/a.b", "https://a.example/aXb"),
            (
                "https://user.example/*",
                "https://evil.example@user.example/",
            ),
        ];
        for (source, address) in cases {
            let pattern = pattern(source);
            let parsed = url(address);
            // The bundle reads `location.href` without its fragment, as the URL parser writes it.
            let href = parsed.as_str().split('#').next().unwrap().to_owned();
            let regex = regex::Regex::new(&pattern.js_regex()).unwrap();
            assert_eq!(
                regex.is_match(&href),
                pattern.matches(&parsed),
                "{source} on {address} ({})",
                pattern.js_regex()
            );
        }
    }

    #[test]
    fn origin_rules_cover_what_the_patterns_name() {
        assert_eq!(
            pattern("https://*.youtube.com/*").origin_rules(),
            ["https://youtube.com", "https://*.youtube.com"]
        );
        assert_eq!(
            pattern("*://m.youtube.com/*").origin_rules(),
            ["https://m.youtube.com", "http://m.youtube.com"]
        );
        assert_eq!(pattern("https://*/*").origin_rules(), ["*"]);
        let patterns = [pattern("https://a.example/*"), pattern("*://*/x")];
        assert_eq!(origin_rules(&patterns), ["*"]);
        let patterns = [
            pattern("https://a.example/*"),
            pattern("https://a.example/b"),
        ];
        assert_eq!(origin_rules(&patterns), ["https://a.example"]);
    }

    fn scripts(json: serde_json::Value) -> Result<SiteScripts, String> {
        SiteScripts::parse(serde_json::from_value(json).map_err(|error| error.to_string())?)
    }

    #[test]
    fn sections_are_checked() {
        let good = scripts(serde_json::json!({
            "scripts": [{
                "matches": ["https://*.youtube.com/*", "https://www.youtube.com/*"],
                "excludeMatches": ["https://www.youtube.com/embed/*"],
                "js": ["dist/youtube.js"],
                "css": ["styles/a.css"],
                "runAt": "document-start",
                "allFrames": true
            }, {
                "matches": ["https://www.youtube.com/*"],
                "css": ["styles/a.css"]
            }]
        }))
        .unwrap();
        assert_eq!(good.groups[0].run_at, RunAt::DocumentStart);
        assert_eq!(good.groups[1].run_at, RunAt::DocumentIdle);
        assert_eq!(
            good.patterns(),
            ["https://*.youtube.com/*", "https://www.youtube.com/*"]
        );
        assert_eq!(good.files(), ["dist/youtube.js", "styles/a.css"]);
        assert!(good.matches(&url("https://m.youtube.com/")));
        assert!(!good.matches(&url("https://example.com/")));

        for bad in [
            serde_json::json!({ "scripts": [] }),
            serde_json::json!({ "scripts": [{ "matches": [], "js": ["a.js"] }] }),
            serde_json::json!({ "scripts": [{ "matches": ["https://a.example/*"] }] }),
            serde_json::json!({ "scripts": [{ "matches": ["https://a.example/*"], "js": ["../a.js"] }] }),
            serde_json::json!({ "scripts": [{ "matches": ["https://a.example/*"], "js": ["a.ts"] }] }),
            serde_json::json!({ "scripts": [{ "matches": ["https://a.example/*"], "css": ["C:/a.css"] }] }),
            serde_json::json!({ "scripts": [{ "matches": ["nope"], "js": ["a.js"] }] }),
            serde_json::json!({ "scripts": [{ "matches": ["https://a.example/*"], "js": ["a.js"], "runAt": "later" }] }),
            serde_json::json!({ "scripts": [{ "matches": ["https://a.example/*"], "js": ["a.js"], "world": "main" }] }),
        ] {
            assert!(scripts(bad.clone()).is_err(), "{bad}");
        }
    }

    #[test]
    fn excluded_pages_get_nothing() {
        let section = scripts(serde_json::json!({
            "scripts": [{
                "matches": ["https://www.example.com/*"],
                "excludeMatches": ["https://www.example.com/embed/*"],
                "js": ["a.js"]
            }]
        }))
        .unwrap();
        assert!(section.matches(&url("https://www.example.com/watch")));
        assert!(!section.matches(&url("https://www.example.com/embed/x")));
    }

    /// A composed bundle run in QuickJS with the little of a page it touches: its scripts run on
    /// a page its patterns match (with their settings and `raygunPage`), not on others, once
    /// however often it's injected, and their offers reach the browser checked.
    fn run_in_page(code: &str, address: &str) -> String {
        use rquickjs::{CatchResultExt, Context, Runtime};
        let page = format!(
            r#"
            var window = globalThis;
            window.top = window;
            var posted = [];
            var location = {{ href: {address:?} }};
            var listeners = [];
            var document = {{
              readyState: 'complete',
              adoptedStyleSheets: [],
              addEventListener: function () {{}},
              documentElement: {{}},
            }};
            window.addEventListener = function () {{}};
            window.raygunBridge = {{ postMessage: function (text) {{ posted.push(JSON.parse(text)); }} }};
            var console = {{ error: function (where, error) {{ posted.push({{ error: String(error) }}); }} }};
            function CSSStyleSheet() {{ this.replaceSync = function (css) {{ this.css = css; }}; }}
            function setTimeout(run) {{ run(); }}
            "#
        );
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            ctx.eval::<(), _>(page).catch(&ctx).unwrap();
            ctx.eval::<(), _>(code).catch(&ctx).unwrap();
            ctx.eval::<(), _>(code).catch(&ctx).unwrap();
            ctx.eval::<String, _>(
                "JSON.stringify({ posted: posted, ran: globalThis.ran || 0, sheets: document.adoptedStyleSheets.map(function (sheet) { return sheet.css; }) })",
            )
            .catch(&ctx)
            .unwrap()
        })
    }

    #[test]
    fn bundles_run_in_pages_their_patterns_match() {
        let section = scripts(serde_json::json!({
            "scripts": [{
                "matches": ["https://*.example.com/*"],
                "excludeMatches": ["https://www.example.com/embed/*"],
                "js": ["dist/a.js"],
                "css": ["a.css"],
                "runAt": "document-start"
            }]
        }))
        .unwrap();
        let files = |path: &str| match path {
            "dist/a.js" => Ok(r#"
                globalThis.ran = (globalThis.ran || 0) + 1;
                raygunPage.offer({ kind: 'video', url: 'https://m.example.com/watch?v=1',
                  title: '  A  video ', artworkUrl: 'javascript:alert(1)', durationMs: 1500.4 });
                raygunPage.offer({ kind: 'program', url: 'https://m.example.com/x' });
                raygunPage.offer({ kind: 'song', url: 'file:///etc/passwd' });
                if (raygunPage.settings.on !== true || raygunPage.platform !== 'android') {
                  throw new Error('settings');
                }
                raygunPage.withdraw();
            "#
            .to_owned()),
            "a.css" => Ok("body { color: red }".to_owned()),
            other => Err(format!("{other} is missing")),
        };
        let mut settings = serde_json::Map::new();
        settings.insert("on".into(), serde_json::json!(true));
        let code = compose(
            "dev.example.site",
            "1.0.0",
            &section,
            files,
            &settings,
            Platform::Android,
        )
        .unwrap();

        let page: serde_json::Value =
            serde_json::from_str(&run_in_page(&code, "https://m.example.com/watch?v=1")).unwrap();
        assert_eq!(page["ran"], 1, "{page}");
        assert_eq!(page["sheets"], serde_json::json!(["body { color: red }\n"]));
        let posted = page["posted"].as_array().unwrap();
        assert_eq!(posted.len(), 2, "{page}");
        assert_eq!(posted[0]["type"], "offer");
        assert_eq!(posted[0]["extension"], "dev.example.site");
        assert_eq!(posted[0]["page"], "https://m.example.com/watch?v=1");
        assert_eq!(posted[0]["offer"]["title"], "A  video");
        assert_eq!(posted[0]["offer"]["durationMs"], 1500);
        assert!(posted[0]["offer"].get("artworkUrl").is_none());
        assert_eq!(posted[1]["type"], "withdraw");

        for elsewhere in ["https://www.example.com/embed/x", "https://example.org/"] {
            let page: serde_json::Value =
                serde_json::from_str(&run_in_page(&code, elsewhere)).unwrap();
            assert_eq!(page["ran"], 0, "{elsewhere}");
            assert_eq!(page["posted"], serde_json::json!([]), "{elsewhere}");
        }
    }

    #[test]
    fn bundles_hold_each_file_behind_the_checks() {
        let section = scripts(serde_json::json!({
            "scripts": [{
                "matches": ["https://*.example.com/*"],
                "js": ["dist/a.js", "dist/b.js"],
                "css": ["a.css"],
                "runAt": "document-start"
            }]
        }))
        .unwrap();
        let files = |path: &str| match path {
            "dist/a.js" => Ok("console.log(raygunPage.settings.on) // no newline".to_owned()),
            "dist/b.js" => Ok("var b = 1;".to_owned()),
            "a.css" => Ok("body { color: red }".to_owned()),
            other => Err(format!("{other} is missing")),
        };
        let mut settings = serde_json::Map::new();
        settings.insert("on".into(), serde_json::json!(true));
        let code = compose(
            "dev.example.site",
            "1.2.0",
            &section,
            files,
            &settings,
            Platform::Android,
        )
        .unwrap();
        assert!(code.starts_with("// dev.example.site 1.2.0"));
        assert!(code.contains("var SETTINGS = {\"on\":true};"));
        assert!(code.contains("var PLATFORM = \"android\";"));
        assert!(code.contains("console.log(raygunPage.settings.on) // no newline\n},"));
        assert!(code.contains("\"body { color: red }\\n\""));
        assert!(code.contains(
            &serde_json::to_string(&pattern("https://*.example.com/*").js_regex()).unwrap()
        ));
        // A file that's missing fails the bundle rather than leaving a script out.
        let missing = compose(
            "dev.example.site",
            "1.2.0",
            &section,
            |_: &str| Err("dist/a.js is missing".to_owned()),
            &settings,
            Platform::Desktop,
        );
        assert_eq!(missing.unwrap_err(), "dist/a.js is missing");
    }
}
