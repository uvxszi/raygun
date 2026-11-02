//! A Shosetsu script as a `novel.source` provider (M32). Each script gets a sandboxed Lua 5.2
//! state on its own thread (its HTTP calls block); the host thread sends it calls and awaits the
//! answers. The extension contract follows kotlin-lib's `LuaExtension` (GPL-3.0).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use mlua::{Function, HookTriggers, Lua, LuaOptions, StdLib, Table, Value, VmState};
use raygun_model::ProviderInfo;
use serde_json::json;
use tokio::sync::oneshot;

use super::http::Fetcher;
use crate::cookies::CookieJar;
use crate::host::ExtensionSource;
use crate::manifest::Manifest;
use crate::novel::NOVEL_SOURCE;

const PRELUDE: &str = include_str!("prelude.lua");
const MEMORY_BYTES: usize = 64 * 1024 * 1024;
/// Longest a script may run for one call, waiting on the network included. Scripts that page
/// through long chapter lists (some pause a second between pages) need a while.
pub const CALL_BUDGET: Duration = Duration::from_secs(80);
/// How long a parsed novel is reused, so `details` and `chapters` parse it once.
const PARSE_REUSE: Duration = Duration::from_secs(60);
/// The provider's local id.
pub const PROVIDER: &str = "novel";

struct Job {
    method: String,
    args: serde_json::Value,
    reply: oneshot::Sender<Result<serde_json::Value, String>>,
}

pub struct ShosetsuExtension {
    manifest: Manifest,
    jobs: mpsc::Sender<Job>,
    no_settings: BTreeMap<String, serde_json::Value>,
    /// The site the script reads (its `baseURL`), for "Open the website" (M46.11).
    website: Option<String>,
}

impl ShosetsuExtension {
    /// Starts the script's thread and waits for the script to load.
    pub async fn load(
        manifest: Manifest,
        source: ExtensionSource,
        cookies: Arc<CookieJar>,
    ) -> Result<Self, String> {
        Self::load_with(manifest, source, cookies, false, CALL_BUDGET).await
    }

    /// Like [`Self::load`]; `loopback` lets the script reach 127.0.0.1 (tests only), and
    /// `budget` is the time one call may take.
    pub async fn load_with(
        manifest: Manifest,
        source: ExtensionSource,
        cookies: Arc<CookieJar>,
        loopback: bool,
        budget: Duration,
    ) -> Result<Self, String> {
        let (jobs, inbox) = mpsc::channel::<Job>();
        let (ready, loaded) = oneshot::channel();
        let entry = manifest.entry.clone();
        let label = manifest.id.clone();
        std::thread::Builder::new()
            .name(format!("shosetsu {label}"))
            .spawn(move || {
                let script = match Script::load(&label, &source, &entry, cookies, loopback, budget)
                {
                    Ok(script) => {
                        let _ = ready.send(Ok(script.website.clone()));
                        script
                    }
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                for job in inbox {
                    let result = script.call(&job.method, &job.args);
                    let _ = job.reply.send(result);
                }
            })
            .map_err(|error| error.to_string())?;
        let website = loaded
            .await
            .map_err(|_| "the script stopped while loading".to_owned())??;
        Ok(Self {
            manifest,
            jobs,
            no_settings: BTreeMap::new(),
            website,
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn website(&self) -> Option<&str> {
        self.website.as_deref()
    }

    pub fn user_settings(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.no_settings
    }

    pub fn providers(&self) -> Vec<ProviderInfo> {
        vec![ProviderInfo {
            id: format!("{}/{PROVIDER}", self.manifest.id),
            extension_id: self.manifest.id.clone(),
            kind: NOVEL_SOURCE.to_owned(),
            name: self.manifest.name.clone(),
        }]
    }

    pub async fn call(
        &self,
        local_id: &str,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        if local_id != PROVIDER {
            return Err(format!("no provider {local_id}"));
        }
        let (reply, answer) = oneshot::channel();
        self.jobs
            .send(Job {
                method: method.to_owned(),
                args: args.clone(),
                reply,
            })
            .map_err(|_| "the script has stopped".to_owned())?;
        answer
            .await
            .map_err(|_| "the script stopped during the call".to_owned())?
    }
}

/// The script's Lua state and the table it returned.
struct Script {
    lua: Lua,
    source: Table,
    deadline: Rc<Cell<Instant>>,
    budget: Duration,
    parsed: RefCell<HashMap<String, (Instant, serde_json::Value)>>,
    website: Option<String>,
}

/// The first line of a Lua error (without the traceback).
fn message(error: &mlua::Error) -> String {
    let text = error.to_string();
    let first = text.lines().next().unwrap_or_default();
    first
        .trim_start_matches("runtime error: ")
        .trim()
        .to_owned()
}

impl Script {
    fn load(
        label: &str,
        source: &ExtensionSource,
        entry: &str,
        cookies: Arc<CookieJar>,
        loopback: bool,
        budget: Duration,
    ) -> Result<Self, String> {
        let code = source.read(entry)?;
        let fetcher = Arc::new(Fetcher::new(loopback, cookies)?);
        let lua = Lua::new_with(
            StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::BIT,
            LuaOptions::default(),
        )
        .map_err(|error| error.to_string())?;
        lua.set_memory_limit(MEMORY_BYTES)
            .map_err(|error| error.to_string())?;
        let deadline = Rc::new(Cell::new(Instant::now() + budget));
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
        let script = Self {
            source: lua.create_table().map_err(|error| error.to_string())?,
            lua,
            deadline,
            budget,
            parsed: RefCell::new(HashMap::new()),
            website: None,
        };
        script
            .install(label, source.clone(), fetcher)
            .map_err(|error| message(&error))?;
        let table: Table = script
            .lua
            .load(lua52(&code).as_ref())
            .set_name(format!("ext({label})"))
            .call(())
            .map_err(|error| message(&error))?;
        for (key, kind) in [
            ("name", "string"),
            ("listings", "table"),
            ("getPassage", "function"),
            ("parseNovel", "function"),
        ] {
            let value: Value = table.get(key).map_err(|error| error.to_string())?;
            if value.type_name() != kind {
                return Err(format!("the script has no {key} ({kind})"));
            }
        }
        let website = table
            .get::<Option<String>>("baseURL")
            .ok()
            .flatten()
            .filter(|url| url.starts_with("https://") || url.starts_with("http://"));
        Ok(Self {
            source: table,
            website,
            ..script
        })
    }

    /// The sandbox: no io, os, package or debug; text-only `load`; Shosetsu's globals.
    fn install(
        &self,
        label: &str,
        source: ExtensionSource,
        fetcher: Arc<Fetcher>,
    ) -> mlua::Result<()> {
        let lua = &self.lua;
        let globals = lua.globals();
        for name in ["dofile", "loadfile", "require", "module"] {
            globals.set(name, Value::Nil)?;
        }
        let string: Table = globals.get("string")?;
        string.set("dump", Value::Nil)?;
        lua.load(
            "local load = load\n\
             _G.load = function(chunk, name, _, env) return load(chunk, name, 't', env) end",
        )
        .set_name("sandbox")
        .exec()?;
        let name = label.to_owned();
        globals.set(
            "print",
            lua.create_function(move |_, values: mlua::Variadic<Value>| {
                let text: Vec<String> = values.iter().map(|value| format!("{value:?}")).collect();
                tracing::debug!(script = %name, "{}", text.join(" "));
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
                let safe = !name.is_empty()
                    && name.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
                    })
                    && !name.starts_with('.');
                if !safe {
                    return Err(mlua::Error::runtime(format!("Missing Library: {name}")));
                }
                let code = source
                    .read(&format!("lib/{name}.lua"))
                    .map_err(|_| mlua::Error::runtime(format!("Missing Library: {name}")))?;
                let value: Value = lua
                    .load(lua52(&code).as_ref())
                    .set_name(format!("lib({name})"))
                    .call(())?;
                libraries.borrow_mut().insert(name, value.clone());
                Ok(value)
            })?,
        )?;
        // os: the clock only.
        let os = lua.create_table()?;
        os.set(
            "time",
            lua.create_function(|_, table: Option<Table>| {
                if table.is_some() {
                    return Err(mlua::Error::runtime("os.time with a date isn't supported"));
                }
                Ok(std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_secs())
                    .unwrap_or_default())
            })?,
        )?;
        let started = Instant::now();
        os.set(
            "clock",
            lua.create_function(move |_, ()| Ok(started.elapsed().as_secs_f64()))?,
        )?;
        os.set(
            "date",
            lua.create_function(|_, (_format, _time): (Option<String>, Option<i64>)| {
                Ok(std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_secs().to_string())
                    .unwrap_or_default())
            })?,
        )?;
        globals.set("os", os)?;
        super::dom::install(lua, &globals)?;
        super::http::install(lua, &globals, fetcher)?;
        lua.load(PRELUDE).set_name("prelude").exec()
    }

    fn call(&self, method: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
        self.deadline.set(Instant::now() + self.budget);
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
        let result = match method {
            "popular" => self.listing(1, page),
            "latest" => self.listing(2, page),
            "search" => self.search(&text("query"), page),
            "details" => self
                .parse(&text("id"))
                .map(|novel| novel["details"].clone()),
            "chapters" => self
                .parse(&text("id"))
                .map(|novel| json!({ "chapters": novel["chapters"].clone() })),
            "passage" => self.passage(&text("chapterId")),
            other => return Err(format!("{other} isn't a novel source call")),
        };
        result.map_err(|error| message(&error))
    }

    /// The call data scripts read: filter defaults, the page and the query.
    fn data(&self, page: u32, query: &str) -> mlua::Result<Table> {
        let data = self.lua.create_table()?;
        if let Ok(filters) = self.source.get::<Table>("searchFilters") {
            defaults(&data, &filters)?;
        }
        let start: i64 = self.source.get::<Option<i64>>("startIndex")?.unwrap_or(1);
        data.set(1, start + i64::from(page) - 1)?;
        data.set(0, query)?;
        Ok(data)
    }

    fn listing(&self, preferred: usize, page: u32) -> mlua::Result<serde_json::Value> {
        let listings: Table = self.source.get("listings")?;
        let count = listings.raw_len();
        if count == 0 {
            return Ok(json!({ "items": [], "hasNextPage": false }));
        }
        let listing: Table = listings.get(preferred.min(count))?;
        let increments: bool = listing.get::<Option<bool>>("increments")?.unwrap_or(false);
        if !increments && page > 1 {
            return Ok(json!({ "items": [], "hasNextPage": false }));
        }
        let func: Function = listing.get("func")?;
        let novels: Value = func.call(self.data(page, "")?)?;
        let items = novel_items(&novels)?;
        let more = increments && !items.is_empty();
        Ok(json!({ "items": items, "hasNextPage": more }))
    }

    fn search(&self, query: &str, page: u32) -> mlua::Result<serde_json::Value> {
        let searchable: bool = self
            .source
            .get::<Option<bool>>("hasSearch")?
            .unwrap_or(true);
        let Some(search) = self
            .source
            .get::<Option<Function>>("search")?
            .filter(|_| searchable)
        else {
            return Err(mlua::Error::runtime("This source can't search"));
        };
        let increments: bool = self
            .source
            .get::<Option<bool>>("isSearchIncrementing")?
            .unwrap_or(true);
        if !increments && page > 1 {
            return Ok(json!({ "items": [], "hasNextPage": false }));
        }
        let novels: Value = search.call(self.data(page, query)?)?;
        let items = novel_items(&novels)?;
        let more = increments && !items.is_empty();
        Ok(json!({ "items": items, "hasNextPage": more }))
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
        let chapters: Vec<serde_json::Value> = sequence(&novel.get("chapters")?)
            .iter()
            .filter_map(|chapter| {
                let Value::Table(chapter) = chapter else {
                    return None;
                };
                let link = string(&chapter.get("link").ok()?)?;
                let order = match chapter.get::<Value>("order").ok()? {
                    Value::Integer(order) => Some(order as f64),
                    Value::Number(order) => Some(order),
                    _ => None,
                };
                Some(json!({
                    "id": link,
                    "name": string(&chapter.get("title").ok()?),
                    "number": order,
                }))
            })
            .collect();
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
        let result = json!({ "details": details, "chapters": chapters });
        self.parsed
            .borrow_mut()
            .insert(id.to_owned(), (Instant::now(), result.clone()));
        Ok(result)
    }

    fn passage(&self, id: &str) -> mlua::Result<serde_json::Value> {
        let passage: Function = self.source.get("getPassage")?;
        let text: String = passage.call(id)?;
        let html = integer(&self.source.get::<Value>("chapterType")?) == Some(1);
        Ok(if html {
            json!({ "html": text, "baseUrl": self.expand(id, 2) })
        } else {
            json!({ "text": text })
        })
    }

    /// The full URL of a shrunk link, when the script can tell.
    fn expand(&self, link: &str, kind: i64) -> Option<String> {
        let expand: Function = self.source.get::<Option<Function>>("expandURL").ok()??;
        let url: String = expand.call((link, kind)).ok()?;
        (url.starts_with("http://") || url.starts_with("https://")).then_some(url)
    }
}

/// LuaJ, which Shosetsu runs, accepts Lua 5.3's `\u{XXXX}` string escapes; Lua 5.2 doesn't.
/// They become the decimal escapes of their UTF-8 bytes.
fn lua52(code: &str) -> std::borrow::Cow<'_, str> {
    if !code.contains("\\u{") {
        return code.into();
    }
    let chars: Vec<char> = code.chars().collect();
    let mut out = String::with_capacity(code.len());
    let mut index = 0;
    while index < chars.len() {
        // An odd run of backslashes ending here starts an escape; an even one is escaped itself.
        let escape_start = chars[index] == '\\'
            && chars[..index]
                .iter()
                .rev()
                .take_while(|c| **c == '\\')
                .count()
                % 2
                == 0;
        if escape_start
            && chars.get(index + 1) == Some(&'u')
            && chars.get(index + 2) == Some(&'{')
            && let Some(close) = chars[index + 3..].iter().position(|c| *c == '}')
        {
            let hex: String = chars[index + 3..index + 3 + close].iter().collect();
            if let Some(decoded) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                let mut buffer = [0u8; 4];
                for byte in decoded.encode_utf8(&mut buffer).bytes() {
                    out.push_str(&format!("\\{byte}"));
                }
                index += 3 + close + 1;
                continue;
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    out.into()
}

/// Fills `data` with each filter's default value, by id.
fn defaults(data: &Table, filters: &Table) -> mlua::Result<()> {
    for filter in filters.clone().sequence_values::<Value>() {
        let Value::Table(filter) = filter? else {
            continue;
        };
        let kind: Option<String> = filter.get("kind")?;
        match kind.as_deref() {
            Some("list" | "group") => {
                if let Ok(inner) = filter.get::<Table>("filters") {
                    defaults(data, &inner)?;
                }
            }
            Some(kind) => {
                let Some(id) = filter.get::<Option<i64>>("id")? else {
                    continue;
                };
                match kind {
                    "text" | "password" => data.set(id, "")?,
                    "switch" | "checkbox" => data.set(id, false)?,
                    "tristate" | "dropdown" | "radio" => data.set(id, 0)?,
                    _ => {}
                }
            }
            None => {}
        }
    }
    Ok(())
}

/// A whole Lua number (LuaJ hands out integers as floats).
fn integer(value: &Value) -> Option<i64> {
    match value {
        Value::Integer(number) => Some(*number),
        Value::Number(number) if number.fract() == 0.0 => Some(*number as i64),
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

/// Novels a listing or search returned, as catalogue items.
fn novel_items(value: &Value) -> mlua::Result<Vec<serde_json::Value>> {
    Ok(sequence(value)
        .iter()
        .filter_map(|novel| {
            let Value::Table(novel) = novel else {
                return None;
            };
            let link = string(&novel.get("link").ok()?)?;
            Some(json!({
                "id": link,
                "title": string(&novel.get("title").ok()?).unwrap_or_else(|| link.clone()),
                "coverUrl": string(&novel.get("imageURL").ok()?),
            }))
        })
        .collect())
}

#[cfg(test)]
pub(super) fn lua52_for_tests(code: &str) -> String {
    lua52(code).into_owned()
}
