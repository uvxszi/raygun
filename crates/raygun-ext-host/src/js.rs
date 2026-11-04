//! The `js` runtime: one QuickJS runtime and context per extension (ARCHITECTURE §7.2).
//!
//! Everything here runs on the host thread (see `host.rs`), so JS values and the `Rc` state
//! behind the native functions never cross threads.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use raygun_model::ProviderInfo;
use reqwest::Url;
use rquickjs::prelude::Async;
use rquickjs::{
    AsyncContext, AsyncRuntime, CatchResultExt, CaughtError, Ctx, Function, Object, Promise,
};
use serde::Deserialize;
use serde_json::json;

use crate::content::ContentFilter;
use crate::cookies::CookieJar;
use crate::html;
use crate::manifest::{Manifest, Runtime, SettingDef, is_provider_id};
use crate::permissions::{HostPattern, NetPolicy, is_public_ip};
use crate::storage::KvStore;

const PRELUDE: &str = include_str!("prelude.js");
const MAX_REDIRECTS: usize = 10;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// Longest single `raygun.time.sleep`: enough to pace rate-limited APIs, short of the call timeout.
const MAX_SLEEP_MS: f64 = 10_000.0;

#[derive(Debug, Clone)]
pub struct Limits {
    pub memory_bytes: usize,
    pub stack_bytes: usize,
    /// Longest run of synchronous JS before it is interrupted. The budget restarts whenever a
    /// call begins or a host request (`fetch`) completes, so time spent waiting doesn't count.
    pub cpu_budget: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: 64 * 1024 * 1024,
            stack_bytes: 1024 * 1024,
            cpu_budget: Duration::from_secs(5),
        }
    }
}

/// State the native functions share with the extension.
struct Shared {
    manifest: Manifest,
    policy: Arc<NetPolicy>,
    client: reqwest::Client,
    storage: RefCell<KvStore>,
    settings: BTreeMap<String, serde_json::Value>,
    /// Settings the script declared while running (Miru's `registerSetting`), after the
    /// manifest's own.
    registered: Rc<RefCell<Vec<SettingDef>>>,
    providers: RefCell<Vec<ProviderInfo>>,
    deadline: Rc<Cell<Instant>>,
    cpu_budget: Duration,
    content: ContentFilter,
}

impl Shared {
    fn restart_budget(&self) {
        self.deadline.set(Instant::now() + self.cpu_budget);
    }
}

pub struct JsExtension {
    shared: Rc<Shared>,
    // Field order matters: the context must drop before its runtime.
    context: AsyncContext,
    _runtime: AsyncRuntime,
}

impl JsExtension {
    /// Creates the runtime, installs the host API and evaluates the extension's entry script.
    pub async fn load(
        manifest: Manifest,
        code: String,
        storage: KvStore,
        cookies: Arc<CookieJar>,
        settings: BTreeMap<String, serde_json::Value>,
        limits: &Limits,
        content: ContentFilter,
    ) -> Result<Self, String> {
        // A Miru script (M46.x) is checked and rewritten as Miru does before anything runs.
        let miru = match manifest.runtime {
            Runtime::Miru => Some(crate::miru::runtime::Prepared::new(&code)?),
            _ => None,
        };
        let policy = Arc::new(manifest.net.clone());
        let client =
            extension_client(policy.clone(), cookies.clone()).map_err(|error| error.to_string())?;
        let deadline = Rc::new(Cell::new(Instant::now() + limits.cpu_budget));
        let registered = Rc::new(RefCell::new(Vec::new()));
        let shared = Rc::new(Shared {
            manifest,
            policy,
            client,
            storage: RefCell::new(storage),
            settings,
            registered: registered.clone(),
            providers: RefCell::new(Vec::new()),
            deadline: deadline.clone(),
            cpu_budget: limits.cpu_budget,
            content,
        });

        let runtime = AsyncRuntime::new().map_err(|error| error.to_string())?;
        // Drives the extension's own futures (its fetches) whatever call is waiting on them. Without
        // it they're polled by the last call into the runtime: two calls at once (a title's details
        // and episodes) left one waiting forever when the other finished first (M46.45). Ends when
        // the runtime is dropped. The host runs extensions on a `LocalSet`.
        tokio::task::spawn_local(runtime.drive());
        runtime.set_memory_limit(limits.memory_bytes).await;
        runtime.set_max_stack_size(limits.stack_bytes).await;
        runtime
            .set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline.get())))
            .await;
        let context = AsyncContext::full(&runtime)
            .await
            .map_err(|error| error.to_string())?;

        let filename = format!("{}/{}", shared.manifest.id, shared.manifest.entry);
        let natives = shared.clone();
        shared.restart_budget();
        context
            .with(move |ctx| -> Result<(), String> {
                install_natives(&ctx, natives).map_err(|error| error.to_string())?;
                eval_script(&ctx, PRELUDE, "raygun:prelude", true)?;
                match &miru {
                    Some(prepared) => crate::miru::runtime::install(
                        &ctx,
                        prepared,
                        crate::miru::runtime::Hooks {
                            cookies,
                            settings: registered,
                        },
                        &filename,
                    ),
                    None => eval_script(&ctx, &code, &filename, true),
                }
            })
            .await?;

        // Scripts that finish setting up asynchronously (converted Nuclear plugins run their
        // `onLoad`/`onEnable` that way) publish a promise; their providers count once it settles.
        context
            .async_with(async |ctx| -> Result<(), String> {
                let ready: rquickjs::Value = ctx
                    .globals()
                    .get("__rg_ready")
                    .map_err(|error| error.to_string())?;
                if let Some(promise) = ready.into_promise() {
                    promise
                        .into_future::<()>()
                        .await
                        .catch(&ctx)
                        .map_err(|error| format!("setup failed: {error}"))?;
                }
                Ok(())
            })
            .await?;

        Ok(Self {
            shared,
            context,
            _runtime: runtime,
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.shared.manifest
    }

    /// The user's values for this extension's settings (defaults not included).
    pub fn user_settings(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.shared.settings
    }

    /// The settings it has: the manifest's, then those the script registered (Miru).
    pub fn settings(&self) -> Vec<SettingDef> {
        let mut settings = self.shared.manifest.settings.clone();
        for setting in self.shared.registered.borrow().iter() {
            if !settings.iter().any(|known| known.key == setting.key) {
                settings.push(setting.clone());
            }
        }
        settings
    }

    pub fn providers(&self) -> Vec<ProviderInfo> {
        self.shared.providers.borrow().clone()
    }

    /// Calls `method` on one of this extension's providers (its local id) and awaits the result.
    pub async fn call(
        &self,
        provider: &str,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let (provider, method, args) = (provider.to_owned(), method.to_owned(), args.to_string());
        let label = format!("{}/{provider}.{method}", self.shared.manifest.id);
        self.shared.restart_budget();
        self.context
            .async_with(async move |ctx| {
                let call: Function = ctx
                    .globals()
                    .get("__rg_call")
                    .map_err(|error| error.to_string())?;
                let promise: Promise = call
                    .call((provider, method, args))
                    .catch(&ctx)
                    .map_err(|error| call_error(&label, error))?;
                let text: String = promise
                    .into_future::<String>()
                    .await
                    .catch(&ctx)
                    .map_err(|error| call_error(&label, error))?;
                serde_json::from_str(&text).map_err(|error| error.to_string())
            })
            .await
    }
}

/// What a failed provider call tells the user: the thrown message alone. The stack trace, which
/// only helps the extension's author, goes to the log.
fn call_error(label: &str, error: CaughtError<'_>) -> String {
    let full = error.to_string();
    tracing::debug!(call = %label, error = %full, "provider call failed");
    match &error {
        CaughtError::Exception(exception) => exception
            .message()
            .filter(|message| !message.trim().is_empty())
            .unwrap_or(full),
        _ => full,
    }
}

/// Evaluates a script in the global scope; `strict` as Raygun's own code runs, sloppy for
/// scripts written for runtimes that evaluate that way (Miru).
pub(crate) fn eval_script(
    ctx: &Ctx<'_>,
    code: &str,
    filename: &str,
    strict: bool,
) -> Result<(), String> {
    let mut options = rquickjs::context::EvalOptions::default();
    options.global = true;
    options.strict = strict;
    options.filename = Some(filename.to_owned());
    ctx.eval_with_options::<(), _>(code, options)
        .catch(ctx)
        .map_err(|error| error.to_string())
}

/// An HTTP client whose redirects obey the extension's permissions, like its own requests, and
/// that keeps the extension's cookies (M46.11).
fn extension_client(
    policy: Arc<NetPolicy>,
    cookies: Arc<CookieJar>,
) -> reqwest::Result<reqwest::Client> {
    let any_public = policy.patterns().contains(&HostPattern::AnyPublic);
    let resolver = PublicResolver {
        policy: policy.clone(),
    };
    let redirects = reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            attempt.error("too many redirects")
        } else if policy.allows(attempt.url()) {
            attempt.follow()
        } else {
            let denied = denied_message(attempt.url());
            attempt.error(denied)
        }
    });
    let mut builder = raygun_net::client_builder()
        .redirect(redirects)
        .cookie_provider(cookies)
        .timeout(REQUEST_TIMEOUT);
    if any_public {
        builder = builder.dns_resolver(Arc::new(resolver));
    }
    builder.build()
}

/// Resolves names for extensions that may reach any public host (`*`: Miru scripts, Nuclear
/// conversions): addresses on this machine or the local network are dropped, so a public name
/// can't lead there. Hosts the manifest names itself keep every address.
struct PublicResolver {
    policy: Arc<NetPolicy>,
}

impl reqwest::dns::Resolve for PublicResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let named = self.policy.names(name.as_str());
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let found = tokio::net::lookup_host((host.as_str(), 0)).await?;
            let addresses: Vec<std::net::SocketAddr> = found
                .filter(|address| named || is_public_ip(address.ip()))
                .collect();
            if addresses.is_empty() {
                return Err(format!("{host} doesn't resolve to a public address").into());
            }
            Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

fn denied_message(url: &Url) -> String {
    format!(
        "network access to {} is not permitted by the extension's manifest",
        url.host_str().unwrap_or(url.as_str())
    )
}

/// The JSON envelopes natives answer with: `{ value }` or `{ error }`.
pub(crate) fn ok(value: impl serde::Serialize) -> String {
    json!({ "value": value }).to_string()
}

pub(crate) fn err(message: impl std::fmt::Display) -> String {
    json!({ "error": message.to_string() }).to_string()
}

fn install_natives<'js>(ctx: &Ctx<'js>, shared: Rc<Shared>) -> rquickjs::Result<()> {
    let native = Object::new(ctx.clone())?;

    let s = shared.clone();
    native.set(
        "log",
        Function::new(ctx.clone(), move |level: String, message: String| {
            let id = &s.manifest.id;
            match level.as_str() {
                "error" => tracing::error!(extension = %id, "{message}"),
                "warn" => tracing::warn!(extension = %id, "{message}"),
                "debug" => tracing::debug!(extension = %id, "{message}"),
                _ => tracing::info!(extension = %id, "{message}"),
            }
        })?,
    )?;

    let s = shared.clone();
    native.set(
        "fetch",
        Function::new(
            ctx.clone(),
            Async(move |url: String, options: String| {
                let s = s.clone();
                async move {
                    let result = fetch(&s, &url, &options).await;
                    s.restart_budget();
                    match result {
                        Ok(response) => ok(response),
                        Err(message) => err(message),
                    }
                }
            }),
        )?,
    )?;

    let s = shared.clone();
    native.set(
        "sleep",
        Function::new(
            ctx.clone(),
            Async(move |ms: f64| {
                let s = s.clone();
                async move {
                    let ms = if ms.is_finite() {
                        ms.clamp(0.0, MAX_SLEEP_MS)
                    } else {
                        0.0
                    };
                    tokio::time::sleep(Duration::from_millis(ms as u64)).await;
                    s.restart_budget();
                }
            }),
        )?,
    )?;

    native.set(
        "htmlSelect",
        Function::new(
            ctx.clone(),
            |source: String, selector: String, fragment: bool| match html::select(
                &source, &selector, fragment,
            ) {
                Ok(elements) => ok(elements),
                Err(message) => err(message),
            },
        )?,
    )?;

    let s = shared.clone();
    native.set(
        "storageGet",
        Function::new(ctx.clone(), move |key: String| {
            ok(s.storage.borrow().get(&key).cloned().unwrap_or_default())
        })?,
    )?;
    let s = shared.clone();
    native.set(
        "storageSet",
        Function::new(
            ctx.clone(),
            move |key: String, value: String| match serde_json::from_str(&value) {
                Ok(value) => match s.storage.borrow_mut().set(&key, value) {
                    Ok(()) => ok(()),
                    Err(message) => err(message),
                },
                Err(error) => err(error),
            },
        )?,
    )?;
    let s = shared.clone();
    native.set(
        "storageRemove",
        Function::new(ctx.clone(), move |key: String| {
            match s.storage.borrow_mut().remove(&key) {
                Ok(()) => ok(()),
                Err(message) => err(message),
            }
        })?,
    )?;
    let s = shared.clone();
    native.set(
        "storageKeys",
        Function::new(ctx.clone(), move || ok(s.storage.borrow().keys()))?,
    )?;

    let s = shared.clone();
    native.set(
        "settingsGet",
        Function::new(ctx.clone(), move |key: String| {
            let value = s
                .settings
                .get(&key)
                .or_else(|| s.manifest.setting_default(&key))
                .cloned()
                .unwrap_or_default();
            ok(value)
        })?,
    )?;

    let s = shared.clone();
    native.set(
        "showNsfw",
        Function::new(ctx.clone(), move || s.content.show_nsfw())?,
    )?;

    let s = shared;
    native.set(
        "registerProvider",
        Function::new(ctx.clone(), move |meta: String| match register(&s, &meta) {
            Ok(()) => ok(()),
            Err(message) => err(message),
        })?,
    )?;

    ctx.globals().set("__rg", native)
}

/// Provider kinds Nuclear plugins become (`CATEGORY_KINDS` in nuclear-compat).
const NUCLEAR_KINDS: [&str; 3] = ["music.metadata", "music.stream", "lyrics"];

#[derive(Deserialize)]
struct ProviderMeta {
    kind: Option<String>,
    id: Option<String>,
    name: Option<String>,
    /// A video source's own Search pill (M46.35).
    #[serde(rename = "searchPill")]
    search_pill: Option<String>,
}

fn register(shared: &Shared, meta: &str) -> Result<(), String> {
    let meta: ProviderMeta = serde_json::from_str(meta).map_err(|error| error.to_string())?;
    let manifest = &shared.manifest;
    let kind = meta.kind.ok_or("provider kind is required")?;
    // A converted Nuclear plugin may register a kind its package didn't declare (OmniSource's
    // metadata, Bandcamp's streams); any kind Raygun runs for Nuclear plugins is allowed.
    let nuclear_kind = manifest.nuclear.is_some() && NUCLEAR_KINDS.contains(&kind.as_str());
    if !manifest.provides.contains(&kind) && !nuclear_kind {
        return Err(format!(
            "provider kind {kind:?} is not listed in the manifest's \"provides\""
        ));
    }
    let id = meta.id.ok_or("provider id is required")?;
    if !is_provider_id(&id) {
        return Err(format!("invalid provider id {id:?}"));
    }
    let full_id = format!("{}/{id}", manifest.id);
    let mut providers = shared.providers.borrow_mut();
    if providers.iter().any(|provider| provider.id == full_id) {
        return Err(format!("provider {id:?} is already registered"));
    }
    let name = meta
        .name
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| manifest.name.clone());
    let search_pill = meta
        .search_pill
        .map(|pill| pill.trim().chars().take(24).collect::<String>())
        .filter(|pill| !pill.is_empty() && kind == "video.source");
    providers.push(ProviderInfo {
        id: full_id,
        extension_id: manifest.id.clone(),
        kind,
        name,
        search_pill,
    });
    Ok(())
}

#[derive(Deserialize)]
struct FetchOptions {
    method: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(serde::Serialize)]
struct FetchResponse {
    status: u16,
    url: String,
    headers: BTreeMap<String, String>,
    body: String,
}

async fn fetch(shared: &Shared, url: &str, options: &str) -> Result<FetchResponse, String> {
    let url = Url::parse(url).map_err(|error| format!("invalid URL {url:?}: {error}"))?;
    if !shared.policy.allows(&url) {
        return Err(denied_message(&url));
    }
    let options: FetchOptions =
        serde_json::from_str(options).map_err(|error| format!("invalid fetch options: {error}"))?;
    let method = reqwest::Method::from_bytes(options.method.to_ascii_uppercase().as_bytes())
        .map_err(|_| format!("invalid method {:?}", options.method))?;

    let host = url.host_str().unwrap_or_default().to_owned();
    let mut request = shared.client.request(method, url);
    for (name, value) in &options.headers {
        request = request.header(name, value);
    }
    request = raygun_net::with_user_agent(request, options.headers.keys().map(String::as_str));
    if let Some(body) = options.body {
        request = request.body(body);
    }
    let mut response = request
        .send()
        .await
        .map_err(|error| request_error(&host, &error))?;

    let status = response.status().as_u16();
    let final_url = response.url().to_string();
    let mut headers = BTreeMap::<String, String>::new();
    for (name, value) in response.headers() {
        let value = String::from_utf8_lossy(value.as_bytes());
        headers
            .entry(name.as_str().to_owned())
            .and_modify(|existing| {
                existing.push_str(", ");
                existing.push_str(&value);
            })
            .or_insert_with(|| value.into_owned());
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(format!(
            "response is larger than {MAX_RESPONSE_BYTES} bytes"
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| request_error(&host, &error))?
    {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(format!(
                "response is larger than {MAX_RESPONSE_BYTES} bytes"
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(FetchResponse {
        status,
        url: final_url,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

/// A short reason for a failed request. reqwest's own message repeats the whole URL; the innermost
/// cause ("operation timed out", a denied redirect) is what the user needs.
fn request_error(host: &str, error: &reqwest::Error) -> String {
    if error.is_timeout() {
        return format!("{host} didn't answer in time");
    }
    let mut cause: &dyn std::error::Error = error;
    while let Some(inner) = cause.source() {
        cause = inner;
    }
    format!("could not reach {host}: {cause}")
}
