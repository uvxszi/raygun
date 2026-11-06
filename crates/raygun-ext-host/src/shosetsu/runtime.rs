//! A Shosetsu script as a `novel.source` provider (M32). Each script gets a sandboxed Lua 5.2
//! state on its own thread (its HTTP calls block); the host thread sends it calls and awaits the
//! answers. The extension contract follows kotlin-lib's `LuaExtension` (GPL-3.0), and how calls
//! map onto it follows the Shosetsu app (GPL-3.0): listings and searches get the filters' data,
//! an empty search shows a listing, paging stops when a page comes back again, and string
//! chapters are one paragraph per line.
//!
//! A script's state is dropped after a while without calls and loaded again on the next one, so
//! many installed scripts cost little but a parked thread each.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use mlua::{Function, HookTriggers, Lua, LuaOptions, StdLib, Table, Value, VmState};
use raygun_model::ProviderInfo;
use serde_json::json;
use tokio::sync::oneshot;

use super::errors;
use super::filters::{self, Spec};
use super::http::Fetcher;
use super::lualib;
use crate::cookies::CookieJar;
use crate::failure::Failure;
use crate::host::ExtensionSource;
use crate::manifest::{Manifest, SettingDef};
use crate::novel::NOVEL_SOURCE;

const PRELUDE: &str = include_str!("prelude.lua");
const MEMORY_BYTES: usize = 128 * 1024 * 1024;
/// Longest a script may run for one call, waiting on the network included. Scripts that page
/// through long chapter lists (some pause a second between pages) need a while.
pub const CALL_BUDGET: Duration = Duration::from_secs(80);
/// How long a parsed novel is reused, so `details` and `chapters` parse it once.
const PARSE_REUSE: Duration = Duration::from_secs(60);
/// How long a script's state stays loaded after its last call.
const IDLE: Duration = Duration::from_secs(5 * 60);
/// How long it stays after loading when no call comes: the app loads every installed script at
/// start, and most aren't used right away.
const IDLE_AFTER_LOAD: Duration = Duration::from_secs(30);
/// The provider's local id.
pub const PROVIDER: &str = "novel";

/// Names that tell which listing is the popular one, and which the latest (lower case).
const POPULAR_NAMES: [&str; 11] = [
    "popular",
    "top",
    "rank",
    "trend",
    "hot",
    "best",
    "rating",
    "views",
    "популяр",
    "рейтинг",
    "人気",
];
const LATEST_NAMES: [&str; 12] = [
    "latest",
    "new",
    "recent",
    "update",
    "release",
    "последн",
    "новые",
    "обновлен",
    "最新",
    "récent",
    "nuevo",
    "últim",
];

struct Job {
    method: String,
    args: serde_json::Value,
    reply: oneshot::Sender<Result<serde_json::Value, Failure>>,
}

pub struct ShosetsuExtension {
    /// Its manifest, with the script's own settings (its `settings` model) as the settings.
    manifest: Manifest,
    jobs: mpsc::Sender<Job>,
    /// The stored values of those settings that still fit them.
    user_settings: BTreeMap<String, serde_json::Value>,
    /// The site the script reads (its `baseURL`), for "Open the website" (M46.11).
    website: Option<String>,
}

/// What a worker needs to load (and reload) its script.
struct Setup {
    label: String,
    name: String,
    entry: String,
    source: ExtensionSource,
    cookies: Arc<CookieJar>,
    /// The user's values for the script's settings, by setting id.
    settings: BTreeMap<String, serde_json::Value>,
    loopback: bool,
    budget: Duration,
}

/// What a loaded script tells the app about itself.
struct Loaded {
    website: Option<String>,
    settings: Vec<SettingDef>,
}

impl ShosetsuExtension {
    /// Starts the script's thread and waits for the script to load. `settings` are the user's
    /// values for the script's settings.
    pub async fn load(
        manifest: Manifest,
        source: ExtensionSource,
        cookies: Arc<CookieJar>,
        settings: BTreeMap<String, serde_json::Value>,
    ) -> Result<Self, String> {
        Self::load_with(manifest, source, cookies, settings, false, CALL_BUDGET).await
    }

    /// Like [`Self::load`]; `loopback` lets the script reach 127.0.0.1 (tests only), and
    /// `budget` is the time one call may take.
    pub async fn load_with(
        mut manifest: Manifest,
        source: ExtensionSource,
        cookies: Arc<CookieJar>,
        settings: BTreeMap<String, serde_json::Value>,
        loopback: bool,
        budget: Duration,
    ) -> Result<Self, String> {
        let (jobs, inbox) = mpsc::channel::<Job>();
        let (ready, loaded) = oneshot::channel();
        let setup = Setup {
            label: manifest.id.clone(),
            name: manifest.name.clone(),
            entry: manifest.entry.clone(),
            source,
            cookies,
            settings: settings.clone(),
            loopback,
            budget,
        };
        std::thread::Builder::new()
            .name(format!("shosetsu {}", manifest.id))
            .spawn(move || work(&setup, &inbox, ready))
            .map_err(|error| error.to_string())?;
        let loaded = loaded
            .await
            .map_err(|_| "the script stopped while loading".to_owned())??;
        manifest.settings = loaded.settings;
        let user_settings = settings
            .into_iter()
            .filter(|(key, value)| manifest.check_setting(key, value).is_ok())
            .collect();
        Ok(Self {
            manifest,
            jobs,
            user_settings,
            website: loaded.website,
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn website(&self) -> Option<&str> {
        self.website.as_deref()
    }

    pub fn user_settings(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.user_settings
    }

    pub fn providers(&self) -> Vec<ProviderInfo> {
        vec![ProviderInfo {
            adult: false,
            id: format!("{}/{PROVIDER}", self.manifest.id),
            extension_id: self.manifest.id.clone(),
            kind: NOVEL_SOURCE.to_owned(),
            name: self.manifest.name.clone(),
            search_pill: None,
        }]
    }

    pub async fn call(
        &self,
        local_id: &str,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.call_reporting(local_id, method, args)
            .await
            .map_err(|failure| failure.message)
    }

    /// Like [`Self::call`]; a failure also carries the request a bot check answered meanwhile.
    pub(crate) async fn call_reporting(
        &self,
        local_id: &str,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, Failure> {
        if local_id != PROVIDER {
            return Err(Failure::said(format!("no provider {local_id}")));
        }
        let (reply, answer) = oneshot::channel();
        self.jobs
            .send(Job {
                method: method.to_owned(),
                args: args.clone(),
                reply,
            })
            .map_err(|_| Failure::said("the script has stopped"))?;
        answer
            .await
            .map_err(|_| Failure::said("the script stopped during the call"))?
    }
}

/// The script's thread: loads it, answers calls, and lets the state go while nothing is asked.
fn work(
    setup: &Setup,
    inbox: &mpsc::Receiver<Job>,
    ready: oneshot::Sender<Result<Loaded, String>>,
) {
    let mut script = match Script::load(setup) {
        Ok(script) => {
            let _ = ready.send(Ok(Loaded {
                website: script.website.clone(),
                settings: script.settings.clone(),
            }));
            Some(script)
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let mut idle = IDLE_AFTER_LOAD;
    loop {
        let job = match inbox.recv_timeout(idle) {
            Ok(job) => {
                idle = IDLE;
                job
            }
            Err(RecvTimeoutError::Timeout) => {
                if script.take().is_some() {
                    tracing::debug!(script = %setup.label, "idle: the script's state is let go");
                }
                match inbox.recv() {
                    Ok(job) => {
                        idle = IDLE;
                        job
                    }
                    Err(_) => return,
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if script.is_none() {
            match Script::load(setup) {
                Ok(loaded) => script = Some(loaded),
                Err(error) => {
                    let _ = job.reply.send(Err(Failure::said(error)));
                    continue;
                }
            }
        }
        if let Some(script) = &script {
            let result = script.call(&job.method, &job.args);
            let _ = job.reply.send(result);
        }
    }
}

/// The script's Lua state and the table it returned.
struct Script {
    lua: Lua,
    source: Table,
    name: String,
    fetcher: Arc<Fetcher>,
    deadline: Rc<Cell<Instant>>,
    budget: Duration,
    parsed: RefCell<HashMap<String, (Instant, serde_json::Value)>>,
    /// The ids each listing or search last served, by what was asked: a page that comes back
    /// the same ends the paging (sites that ignore the page number repeat their first page).
    last_pages: RefCell<HashMap<String, Vec<String>>>,
    website: Option<String>,
    /// The script's settings, as the app shows them.
    settings: Vec<SettingDef>,
}

/// The first line of a Lua error, without its position and traceback.
pub(crate) fn first_line(text: &str) -> String {
    errors::first_line(text)
}

/// Which of the app's two lists is asked for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pick {
    Popular,
    Latest,
}

impl Script {
    fn load(setup: &Setup) -> Result<Self, String> {
        let describe = |error: &mlua::Error| errors::describe(&setup.name, error, None);
        let code = setup.source.read(&setup.entry)?;
        let fetcher = Arc::new(Fetcher::new(setup.loopback, setup.cookies.clone())?);
        let lua = Lua::new_with(
            StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::BIT | StdLib::COROUTINE,
            LuaOptions::default(),
        )
        .map_err(|error| error.to_string())?;
        lua.set_memory_limit(MEMORY_BYTES)
            .map_err(|error| error.to_string())?;
        let deadline = Rc::new(Cell::new(Instant::now() + setup.budget));
        let watched = deadline.clone();
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(10_000),
            move |_, _| {
                if Instant::now() > watched.get() {
                    Err(mlua::Error::runtime("the script ran too long"))
                } else {
                    Ok(VmState::Continue)
                }
            },
        );
        install(&lua, &setup.label, setup.source.clone(), fetcher.clone())
            .map_err(|error| describe(&error))?;
        let table: Table = lualib::load_chunk(&lua, &code, &format!("ext({})", setup.label))
            .and_then(|chunk| chunk.call(()))
            .map_err(|error| describe(&error))?;
        for (key, kind) in [
            ("name", "string"),
            ("listings", "table"),
            ("getPassage", "function"),
            ("parseNovel", "function"),
        ] {
            let value: Value = table.get(key).map_err(|error| error.to_string())?;
            if value.type_name() != kind {
                return Err(format!(
                    "{} isn't a usable Shosetsu script: it has no {key} ({kind})",
                    setup.name
                ));
            }
        }
        let website = table
            .get::<Option<String>>("baseURL")
            .ok()
            .flatten()
            .filter(|url| url.starts_with("https://") || url.starts_with("http://"));
        let settings =
            apply_settings(&lua, &table, &setup.settings).map_err(|error| describe(&error))?;
        Ok(Self {
            lua,
            source: table,
            name: setup.name.clone(),
            fetcher,
            deadline,
            budget: setup.budget,
            parsed: RefCell::new(HashMap::new()),
            last_pages: RefCell::new(HashMap::new()),
            website,
            settings,
        })
    }

    fn call(&self, method: &str, args: &serde_json::Value) -> Result<serde_json::Value, Failure> {
        self.deadline.set(Instant::now() + self.budget);
        self.fetcher.start_call();
        let text = |key: &str| {
            args.get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let page = args
            .get("page")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1)
            .max(1) as u32;
        let chosen = args.get("filters").and_then(serde_json::Value::as_object);
        let result = match method {
            "popular" => self.list(Pick::Popular, page),
            "latest" => self.list(Pick::Latest, page),
            "search" => self.search(&text("query"), page, chosen),
            "filters" => self.filters(),
            "details" => self
                .parse(&text("id"))
                .map(|novel| novel["details"].clone()),
            "chapters" => self
                .parse(&text("id"))
                .map(|novel| json!({ "chapters": novel["chapters"].clone() })),
            "passage" => self.passage(&text("chapterId")),
            // How much memory the state holds (for the load-cost survey).
            #[cfg(test)]
            "memory" => Ok(json!(self.lua.used_memory())),
            other => return Err(Failure::said(format!("{other} isn't a novel source call"))),
        };
        result.map_err(|error| {
            let message =
                errors::describe(&self.name, &error, self.fetcher.challenged().as_deref());
            tracing::debug!(script = %self.name, %method, %error, "a Shosetsu call failed");
            Failure {
                checked: self.fetcher.checked().map(Box::new),
                ..Failure::said(message)
            }
        })
    }

    fn listings(&self) -> mlua::Result<Vec<(String, Table)>> {
        let listings: Table = self.source.get("listings")?;
        Ok(listings
            .sequence_values::<Value>()
            .filter_map(Result::ok)
            .filter_map(|listing| match listing {
                Value::Table(listing) => {
                    let name: String = listing.get("name").unwrap_or_default();
                    Some((name, listing))
                }
                _ => None,
            })
            .collect())
    }

    /// The listing (1-based) that answers "popular" or "latest": by its name, else the first
    /// for popular and the second for latest.
    fn pick(&self, names: &[String], pick: Pick) -> usize {
        let find = |words: &[&str]| {
            names.iter().position(|name| {
                let name = name.to_lowercase();
                words.iter().any(|word| name.contains(word))
            })
        };
        let popular = find(&POPULAR_NAMES).unwrap_or(0);
        let index = match pick {
            Pick::Popular => popular,
            Pick::Latest => find(&LATEST_NAMES)
                .filter(|latest| *latest != popular || names.len() == 1)
                .unwrap_or_else(|| {
                    (0..names.len())
                        .find(|index| *index != popular)
                        .unwrap_or(0)
                }),
        };
        index + 1
    }

    fn list(&self, pick: Pick, page: u32) -> mlua::Result<serde_json::Value> {
        let listings = self.listings()?;
        if listings.is_empty() {
            return Ok(json!({ "items": [], "hasNextPage": false }));
        }
        let names: Vec<String> = listings.iter().map(|(name, _)| name.clone()).collect();
        self.run_listing(&listings, self.pick(&names, pick), page, None)
    }

    /// Runs listing `index` (1-based) for `page`, with the chosen filters when there are some.
    fn run_listing(
        &self,
        listings: &[(String, Table)],
        index: usize,
        page: u32,
        chosen: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> mlua::Result<serde_json::Value> {
        let Some((_, listing)) = listings.get(index.saturating_sub(1)) else {
            return Ok(json!({ "items": [], "hasNextPage": false }));
        };
        let increments: bool = listing.get::<Option<bool>>("increments")?.unwrap_or(false);
        if !increments && page > 1 {
            return Ok(json!({ "items": [], "hasNextPage": false }));
        }
        let func: Function = listing.get("func")?;
        let novels: Value = func.call(self.data(page, None, chosen)?)?;
        let key = format!("listing {index} {}", filter_key(chosen));
        Ok(self.page_answer(&key, page, novel_items(&novels), increments))
    }

    fn search(
        &self,
        query: &str,
        page: u32,
        chosen: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> mlua::Result<serde_json::Value> {
        // Shosetsu shows the chosen listing, with the filters, when nothing is typed.
        if query.trim().is_empty() {
            let listings = self.listings()?;
            if listings.is_empty() {
                return Ok(json!({ "items": [], "hasNextPage": false }));
            }
            let names: Vec<String> = listings.iter().map(|(name, _)| name.clone()).collect();
            let index = chosen
                .and_then(|chosen| filters::chosen_listing(chosen, listings.len()))
                .unwrap_or_else(|| self.pick(&names, Pick::Popular));
            return self.run_listing(&listings, index, page, chosen);
        }
        let searchable: bool = self
            .source
            .get::<Option<bool>>("hasSearch")?
            .unwrap_or(true);
        let Some(search) = self
            .source
            .get::<Option<Function>>("search")?
            .filter(|_| searchable)
        else {
            return Err(mlua::Error::runtime(format!("{} can't search", self.name)));
        };
        let increments: bool = self
            .source
            .get::<Option<bool>>("isSearchIncrementing")?
            .unwrap_or(true);
        if !increments && page > 1 {
            return Ok(json!({ "items": [], "hasNextPage": false }));
        }
        let novels: Value = search.call(self.data(page, Some(query), chosen)?)?;
        let key = format!("search {query} {}", filter_key(chosen));
        Ok(self.page_answer(&key, page, novel_items(&novels), increments))
    }

    /// A page of novels, and whether there may be another: an incrementing list goes on until a
    /// page is empty or repeats the one before (Shosetsu's rule).
    fn page_answer(
        &self,
        key: &str,
        page: u32,
        items: Vec<serde_json::Value>,
        increments: bool,
    ) -> serde_json::Value {
        let ids: Vec<String> = items
            .iter()
            .filter_map(|item| item["id"].as_str().map(str::to_owned))
            .collect();
        let mut last_pages = self.last_pages.borrow_mut();
        let repeated = page > 1 && last_pages.get(key).is_some_and(|last| *last == ids);
        if last_pages.len() > 64 {
            last_pages.clear();
        }
        last_pages.insert(key.to_owned(), ids);
        if repeated {
            return json!({ "items": [], "hasNextPage": false });
        }
        let more = increments && !items.is_empty();
        json!({ "items": items, "hasNextPage": more })
    }

    /// The script's filters for the app, and a listing picker when it has several listings.
    fn filters(&self) -> mlua::Result<serde_json::Value> {
        let names: Vec<String> = self.listings()?.into_iter().map(|(name, _)| name).collect();
        let mut out = Vec::new();
        let shown = self.pick(&names, Pick::Popular);
        filters::describe(&self.filter_specs(), &names, shown, &mut out);
        Ok(serde_json::Value::Array(out))
    }

    fn filter_specs(&self) -> Vec<Spec> {
        self.source
            .get::<Table>("searchFilters")
            .map(|filters| filters::read(&filters))
            .unwrap_or_default()
    }

    /// The call data scripts read, as Shosetsu builds it: every filter's value by id (defaults,
    /// then what was chosen), the page, and the query when searching.
    fn data(
        &self,
        page: u32,
        query: Option<&str>,
        chosen: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> mlua::Result<Table> {
        let data = self.lua.create_table()?;
        let specs = self.filter_specs();
        filters::defaults(&specs, &data)?;
        if let Some(chosen) = chosen {
            filters::apply(&specs, chosen, &data)?;
        }
        let start: i64 = integer(&self.source.get::<Value>("startIndex")?).unwrap_or(1);
        data.set(1, start + i64::from(page) - 1)?;
        if let Some(query) = query {
            data.set(0, query)?;
        }
        Ok(data)
    }

    /// One `parseNovel(link, true)`, kept for a minute: details and chapters come from it.
    fn parse(&self, id: &str) -> mlua::Result<serde_json::Value> {
        if let Some((at, novel)) = self.parsed.borrow().get(id)
            && at.elapsed() < PARSE_REUSE
        {
            return Ok(novel.clone());
        }
        let parse: Function = self.source.get("parseNovel")?;
        let novel: Table = parse.call((id, true))?;
        let mut genres = strings(&novel.get("genres")?);
        for tag in strings(&novel.get("tags")?) {
            if !genres.contains(&tag) {
                genres.push(tag);
            }
        }
        let status = match integer(&novel.get::<Value>("status")?) {
            Some(0) => "ongoing",
            Some(1) => "completed",
            Some(2) => "hiatus",
            _ => "unknown",
        };
        let details = json!({
            "id": id,
            "title": string(&novel.get("title")?).unwrap_or_else(|| id.to_owned()),
            "coverUrl": string(&novel.get("imageURL")?),
            "authors": strings(&novel.get("authors")?),
            "artists": strings(&novel.get("artists")?),
            "description": string(&novel.get("description")?),
            "genres": genres,
            "status": status,
            "url": self.expand(id, 1),
        });
        let result = json!({ "details": details, "chapters": chapters(&novel.get("chapters")?) });
        let mut parsed = self.parsed.borrow_mut();
        parsed.retain(|_, (at, _)| at.elapsed() < PARSE_REUSE);
        parsed.insert(id.to_owned(), (Instant::now(), result.clone()));
        Ok(result)
    }

    fn passage(&self, id: &str) -> mlua::Result<serde_json::Value> {
        let passage: Function = self.source.get("getPassage")?;
        let text: String = passage.call(id)?;
        let html = integer(&self.source.get::<Value>("chapterType")?) == Some(1);
        let base = self.expand(id, 2);
        // A string chapter reads as Shosetsu shows it: each line a paragraph.
        let html = if html { text } else { lines_as_html(&text) };
        Ok(json!({ "html": html, "baseUrl": base }))
    }

    /// The full URL of a shrunk link, when the script can tell.
    fn expand(&self, link: &str, kind: i64) -> Option<String> {
        if link.starts_with("https://") || link.starts_with("http://") {
            return Some(link.to_owned());
        }
        let expand: Function = self.source.get::<Option<Function>>("expandURL").ok()??;
        let url: String = expand.call((link, kind)).ok()?;
        (url.starts_with("http://") || url.starts_with("https://")).then_some(url)
    }
}

/// The sandbox: no io, os, package or debug; text-only `load`; Shosetsu's globals.
fn install(
    lua: &Lua,
    label: &str,
    source: ExtensionSource,
    fetcher: Arc<Fetcher>,
) -> mlua::Result<()> {
    let globals = lua.globals();
    for name in ["dofile", "loadfile", "require", "module"] {
        globals.set(name, Value::Nil)?;
    }
    let string: Table = globals.get("string")?;
    string.set("dump", Value::Nil)?;
    lua.load(
        "local load = load\n\
         _G.load = function(chunk, name, _, env) return load(chunk, name, 't', env) end\n\
         if loadstring then _G.loadstring = function(chunk, name) return load(chunk, name, 't') end end",
    )
    .set_name("sandbox")
    .exec()?;
    let name = label.to_owned();
    globals.set(
        "print",
        lua.create_function(move |_, values: mlua::Variadic<Value>| {
            let text: Vec<String> = values
                .iter()
                .map(|value| match value {
                    Value::String(text) => text.to_string_lossy(),
                    other => format!("{other:?}"),
                })
                .collect();
            tracing::debug!(script = %name, "{}", text.join(" "));
            #[cfg(test)]
            if std::env::var_os("SHOSETSU_TRACE").is_some() {
                eprintln!("  print: {}", text.join(" "));
            }
            Ok(())
        })?,
    )?;
    globals.set(
        "Log",
        lua.create_function(|_, (name, text): (String, String)| {
            tracing::debug!(script = %name, "{text}");
            Ok(())
        })?,
    )?;
    globals.set(
        "delay",
        lua.create_function(|_, milliseconds: u64| {
            std::thread::sleep(Duration::from_millis(milliseconds.min(10_000)));
            Ok(())
        })?,
    )?;
    let libraries: Rc<RefCell<HashMap<String, Value>>> = Rc::new(RefCell::new(HashMap::new()));
    globals.set(
        "Require",
        lua.create_function(move |lua, name: String| {
            if let Some(loaded) = libraries.borrow().get(&name) {
                return Ok(loaded.clone());
            }
            if !super::package::is_library_name(&name) {
                return Err(mlua::Error::runtime(format!("Missing Library: {name}")));
            }
            let code = source
                .read(&format!("lib/{name}.lua"))
                .map_err(|_| mlua::Error::runtime(format!("Missing Library: {name}")))?;
            let value: Value = lualib::load_chunk(lua, &code, &format!("lib({name})"))?.call(())?;
            if name == "dkjson" {
                lualib::speed_up_dkjson(lua, &value)?;
            }
            libraries.borrow_mut().insert(name, value.clone());
            Ok(value)
        })?,
    )?;
    lualib::install_os(lua, &globals)?;
    lualib::install_string_case(lua, &globals)?;
    super::dom::install(lua, &globals)?;
    super::http::install(lua, &globals, fetcher)?;
    lua.load(PRELUDE).set_name("prelude").exec()
}

/// The script's settings model as extension settings, after handing the script the user's values
/// (else the defaults) through its `updateSetting`, as Shosetsu does.
fn apply_settings(
    lua: &Lua,
    table: &Table,
    stored: &BTreeMap<String, serde_json::Value>,
) -> mlua::Result<Vec<SettingDef>> {
    let specs = table
        .get::<Table>("settings")
        .map(|model| filters::read(&model))
        .unwrap_or_default();
    let mut defs = Vec::new();
    filters::settings(&specs, &mut defs);
    let Some(update) = table.get::<Option<Function>>("updateSetting")? else {
        return Ok(defs);
    };
    for def in &defs {
        let value = stored
            .get(&def.key)
            .filter(|value| def.accepts(value))
            .or(def.default.as_ref());
        let Some((id, value)) =
            value.and_then(|value| filters::setting_value(&specs, &def.key, value))
        else {
            continue;
        };
        let value = match value {
            serde_json::Value::Bool(flag) => Value::Boolean(flag),
            serde_json::Value::Number(number) => Value::Integer(number.as_i64().unwrap_or(0)),
            serde_json::Value::String(text) => Value::String(lua.create_string(text)?),
            _ => continue,
        };
        // A setting the script can't take leaves it as it was, rather than not loading.
        if let Err(error) = update.call::<()>((id, value)) {
            tracing::warn!(%error, setting = %def.key, "a Shosetsu script refused a setting");
        }
    }
    Ok(defs)
}

/// The filters chosen for a call, as a key for its pages.
fn filter_key(chosen: Option<&serde_json::Map<String, serde_json::Value>>) -> String {
    chosen
        .map(|chosen| serde_json::Value::Object(chosen.clone()).to_string())
        .unwrap_or_default()
}

/// A string chapter as HTML: each line a paragraph, escaped (Shosetsu's `asHtml`).
fn lines_as_html(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            format!(
                "<p>{}</p>",
                line.replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
            )
        })
        .collect()
}

/// LuaJ, which Shosetsu runs, reads string escapes more loosely than Lua 5.2: it accepts Lua
/// 5.3's `\u{XXXX}`, and an unknown escape such as `"\s"` stands for the letter itself (Lua 5.1's
/// rule), where Lua 5.2 refuses to compile. Short strings are rewritten to mean the same in 5.2:
/// `\u{…}` becomes the decimal escapes of its UTF-8 bytes, and an unknown escape loses its
/// backslash. Long strings and comments are left alone.
pub(crate) fn lua52(code: &str) -> std::borrow::Cow<'_, str> {
    if !code.contains('\\') {
        return code.into();
    }
    let chars: Vec<char> = code.chars().collect();
    let mut out = String::with_capacity(code.len());
    let mut index = 0;
    let mut changed = false;
    // `[[`, `[=[`…: the closing bracket's level, when one opens at `at`.
    let long_bracket = |at: usize| -> Option<(usize, usize)> {
        if chars.get(at) != Some(&'[') {
            return None;
        }
        let level = chars[at + 1..].iter().take_while(|c| **c == '=').count();
        (chars.get(at + 1 + level) == Some(&'[')).then_some((level, at + 2 + level))
    };
    let close_of = |from: usize, level: usize| -> usize {
        let mut at = from;
        while at < chars.len() {
            if chars[at] == ']'
                && chars[at + 1..]
                    .iter()
                    .take(level)
                    .filter(|c| **c == '=')
                    .count()
                    == level
                && chars.get(at + 1 + level) == Some(&']')
            {
                return at + 2 + level;
            }
            at += 1;
        }
        chars.len()
    };
    while index < chars.len() {
        let c = chars[index];
        if c == '-' && chars.get(index + 1) == Some(&'-') {
            // A comment: long, or to the end of the line.
            let end = match long_bracket(index + 2) {
                Some((level, body)) => close_of(body, level),
                None => chars[index..]
                    .iter()
                    .position(|c| *c == '\n')
                    .map_or(chars.len(), |end| index + end),
            };
            out.extend(&chars[index..end]);
            index = end;
            continue;
        }
        if let Some((level, body)) = long_bracket(index) {
            let end = close_of(body, level);
            out.extend(&chars[index..end]);
            index = end;
            continue;
        }
        if c != '"' && c != '\'' {
            out.push(c);
            index += 1;
            continue;
        }
        // A short string: copied with its escapes made Lua 5.2's.
        out.push(c);
        index += 1;
        while index < chars.len() {
            let inner = chars[index];
            if inner == c || inner == '\n' {
                out.push(inner);
                index += 1;
                break;
            }
            if inner != '\\' {
                out.push(inner);
                index += 1;
                continue;
            }
            let Some(&next) = chars.get(index + 1) else {
                out.push(inner);
                index += 1;
                break;
            };
            if next == 'u'
                && chars.get(index + 2) == Some(&'{')
                && let Some(close) = chars[index + 3..].iter().position(|c| *c == '}')
                && let Some(decoded) = u32::from_str_radix(
                    &chars[index + 3..index + 3 + close]
                        .iter()
                        .collect::<String>(),
                    16,
                )
                .ok()
                .and_then(char::from_u32)
            {
                let mut buffer = [0u8; 4];
                for byte in decoded.encode_utf8(&mut buffer).bytes() {
                    out.push_str(&format!("\\{byte}"));
                }
                index += 4 + close;
                changed = true;
                continue;
            }
            let known = matches!(
                next,
                'a' | 'b'
                    | 'f'
                    | 'n'
                    | 'r'
                    | 't'
                    | 'v'
                    | '\\'
                    | '"'
                    | '\''
                    | '\n'
                    | '\r'
                    | 'x'
                    | 'z'
            ) || next.is_ascii_digit();
            if !known {
                changed = true;
                out.push(next);
                index += 2;
                continue;
            }
            out.push(inner);
            out.push(next);
            index += 2;
        }
    }
    if changed { out.into() } else { code.into() }
}

/// A whole Lua number (LuaJ hands out integers as floats).
fn integer(value: &Value) -> Option<i64> {
    match value {
        Value::Integer(number) => Some(*number),
        Value::Number(number) if number.fract() == 0.0 => Some(*number as i64),
        Value::String(text) => text.to_str().ok()?.trim().parse().ok(),
        _ => None,
    }
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Integer(number) => Some(*number as f64),
        Value::Number(number) if number.is_finite() => Some(*number),
        // Kotlin coerces a numeric string to the chapter's `order` too.
        Value::String(text) => text
            .to_str()
            .ok()?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite()),
        _ => None,
    }
}

fn string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            Some(text.to_str().ok()?.trim().to_owned()).filter(|text| !text.is_empty())
        }
        Value::Integer(number) => Some(number.to_string()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// A Lua list (or an `Elements`-like userdata with size/get) as values, in order.
fn sequence(value: &Value) -> Vec<Value> {
    match value {
        Value::Table(table) => {
            let mut values: Vec<Value> = table
                .clone()
                .sequence_values::<Value>()
                .filter_map(Result::ok)
                .collect();
            if values.is_empty() {
                // Tables built with pairs-based helpers may not start at 1.
                let mut pairs: Vec<(i64, Value)> = table
                    .clone()
                    .pairs::<i64, Value>()
                    .filter_map(Result::ok)
                    .collect();
                pairs.sort_by_key(|(key, _)| *key);
                values = pairs.into_iter().map(|(_, value)| value).collect();
            }
            values
        }
        _ => Vec::new(),
    }
}

fn strings(value: &Value) -> Vec<String> {
    match value {
        Value::String(_) => string(value).into_iter().collect(),
        _ => sequence(value).iter().filter_map(string).collect(),
    }
}

/// A novel's chapters for the app, newest first (the order the app's "source order" expects):
/// by their `order` when the script numbers them, else the reverse of its list, which Shosetsu
/// scripts give oldest first.
fn chapters(value: &Value) -> Vec<serde_json::Value> {
    let mut chapters: Vec<(usize, Option<f64>, serde_json::Value)> = sequence(value)
        .iter()
        .enumerate()
        .filter_map(|(index, chapter)| {
            let Value::Table(chapter) = chapter else {
                return None;
            };
            let link = string(&chapter.get("link").ok()?)?;
            let order = number(&chapter.get::<Value>("order").ok()?);
            let released = release(&chapter.get::<Value>("release").ok()?);
            Some((
                index,
                order,
                json!({
                    "id": link,
                    "name": string(&chapter.get("title").ok()?),
                    "number": order,
                    "dateUpload": released,
                }),
            ))
        })
        .collect();
    let numbered = {
        let mut orders = chapters.iter().filter_map(|(_, order, _)| *order);
        orders
            .next()
            .is_some_and(|first| orders.any(|other| other != first))
    };
    if numbered {
        chapters.sort_by(|(a_index, a_order, _), (b_index, b_order, _)| {
            b_order
                .unwrap_or(f64::MIN)
                .total_cmp(&a_order.unwrap_or(f64::MIN))
                .then(b_index.cmp(a_index))
        });
    } else {
        chapters.reverse();
    }
    chapters
        .into_iter()
        .map(|(_, _, chapter)| chapter)
        .collect()
}

/// When a chapter came out, in Unix ms, from the dates scripts give: Unix seconds or ms, or a
/// date such as `2024-05-01`, `2024-05-01 10:20:30`, `01.05.2024` or `May 1, 2024`.
fn release(value: &Value) -> Option<i64> {
    use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};
    let from_number = |number: f64| -> Option<i64> {
        match number {
            seconds if (1e9..1e11).contains(&seconds) => Some((seconds * 1000.0) as i64),
            ms if (1e12..1e14).contains(&ms) => Some(ms as i64),
            _ => None,
        }
    };
    let text = match value {
        Value::Integer(number) => return from_number(*number as f64),
        Value::Number(number) => return from_number(*number),
        Value::String(text) => text.to_str().ok()?.trim().to_owned(),
        _ => return None,
    };
    if let Ok(number) = text.parse::<f64>() {
        return from_number(number);
    }
    if let Ok(moment) = chrono::DateTime::parse_from_rfc3339(&text) {
        return Some(moment.timestamp_millis());
    }
    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(moment) = NaiveDateTime::parse_from_str(&text, format) {
            return Some(Utc.from_utc_datetime(&moment).timestamp_millis());
        }
    }
    let date_text = text
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    for format in [
        "%Y-%m-%d",
        "%Y/%m/%d",
        "%d.%m.%Y",
        "%d/%m/%Y",
        "%B %d, %Y",
        "%b %d, %Y",
        "%d %B %Y",
        "%d %b %Y",
    ] {
        let candidate = if format.contains(' ') {
            date_text.as_str()
        } else {
            text.split_whitespace().next().unwrap_or_default()
        };
        if let Ok(date) = NaiveDate::parse_from_str(candidate, format) {
            return date
                .and_hms_opt(0, 0, 0)
                .map(|moment| Utc.from_utc_datetime(&moment).timestamp_millis());
        }
    }
    None
}

/// Novels a listing or search returned, as catalogue items.
fn novel_items(value: &Value) -> Vec<serde_json::Value> {
    let mut seen = std::collections::HashSet::new();
    sequence(value)
        .iter()
        .filter_map(|novel| {
            let Value::Table(novel) = novel else {
                return None;
            };
            let link = string(&novel.get("link").ok()?)?;
            // A page listing a novel twice (a "hot" box and the list) shows it once.
            if !seen.insert(link.clone()) {
                return None;
            }
            Some(json!({
                "id": link,
                "title": string(&novel.get("title").ok()?).unwrap_or_else(|| link.clone()),
                "coverUrl": string(&novel.get("imageURL").ok()?),
            }))
        })
        .collect()
}

#[cfg(test)]
pub(super) fn lua52_for_tests(code: &str) -> String {
    lua52(code).into_owned()
}

#[cfg(test)]
pub(super) fn chapters_for_tests(lua: &Lua, code: &str) -> Vec<serde_json::Value> {
    chapters(&lua.load(code).eval::<Value>().unwrap())
}

#[cfg(test)]
pub(super) fn release_for_tests(text: &str) -> Option<i64> {
    let lua = Lua::new();
    release(&Value::String(lua.create_string(text).unwrap()))
}

#[cfg(test)]
pub(super) fn lines_as_html_for_tests(text: &str) -> String {
    lines_as_html(text)
}
