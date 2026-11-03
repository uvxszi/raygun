//! The parts of LuaJ's standard library Shosetsu scripts get that Raygun's sandbox writes itself:
//! `os` limited to the clock (as kotlin-lib allows: `clock`, `date`, `difftime`, `setlocale`,
//! `time`), a native JSON decoder behind the dkjson library scripts `Require`, and a cache of
//! compiled chunks shared by every script's state.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Timelike, Utc};
use mlua::{Function, Lua, MultiValue, Table, Value};
use sha2::{Digest, Sha256};

/// Most compiled chunks kept; scripts and libraries are a few dozen kilobytes each.
const MAX_CHUNKS: usize = 256;

type Chunks = Mutex<HashMap<[u8; 32], Arc<Vec<u8>>>>;

fn chunks() -> &'static Chunks {
    static CHUNKS: OnceLock<Chunks> = OnceLock::new();
    CHUNKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Loads `code` as a function named `name`, compiling it once per process: other scripts using
/// the same library, and a script loaded again after it went idle, reuse the bytecode.
pub fn load_chunk(lua: &Lua, code: &str, name: &str) -> mlua::Result<Function> {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    hasher.update([0]);
    hasher.update(code.as_bytes());
    let key: [u8; 32] = hasher.finalize().into();
    let cached = chunks()
        .lock()
        .ok()
        .and_then(|chunks| chunks.get(&key).cloned());
    if let Some(bytecode) = cached {
        // Our own bytecode, made below from this very text: scripts still can't load binary.
        if let Ok(function) = lua
            .load(&bytecode[..])
            .set_name(name)
            .set_mode(mlua::ChunkMode::Binary)
            .into_function()
        {
            return Ok(function);
        }
    }
    let function = lua
        .load(super::runtime::lua52(code).as_ref())
        .set_name(name)
        .set_mode(mlua::ChunkMode::Text)
        .into_function()?;
    if let Ok(mut chunks) = chunks().lock() {
        if chunks.len() >= MAX_CHUNKS {
            chunks.clear();
        }
        chunks.insert(key, Arc::new(function.dump(false)));
    }
    Ok(function)
}

/// `os`: the clock only.
pub fn install_os(lua: &Lua, globals: &Table) -> mlua::Result<()> {
    let os = lua.create_table()?;
    os.set(
        "time",
        lua.create_function(|_, table: Option<Table>| match table {
            None => Ok(Some(Utc::now().timestamp())),
            Some(table) => time_of(&table),
        })?,
    )?;
    let started = Instant::now();
    os.set(
        "clock",
        lua.create_function(move |_, ()| Ok(started.elapsed().as_secs_f64()))?,
    )?;
    os.set(
        "date",
        lua.create_function(|lua, (format, time): (Option<String>, Option<f64>)| {
            date(lua, format.as_deref().unwrap_or("%c"), time)
        })?,
    )?;
    os.set(
        "difftime",
        lua.create_function(|_, (later, earlier): (f64, Option<f64>)| {
            Ok(later - earlier.unwrap_or(0.0))
        })?,
    )?;
    os.set(
        "setlocale",
        lua.create_function(|_, _: MultiValue| Ok("C"))?,
    )?;
    globals.set("os", os)
}

/// `os.time{year=, month=, day=, hour=12, min=0, sec=0}` in local time, normalized as C's
/// `mktime` does (month 13 is January of the next year).
fn time_of(table: &Table) -> mlua::Result<Option<i64>> {
    let field = |name: &str, default: Option<i64>| -> mlua::Result<i64> {
        match table.get::<Option<f64>>(name)? {
            Some(value) => Ok(value.floor() as i64),
            None => default.ok_or_else(|| {
                mlua::Error::runtime(format!("field '{name}' missing in date table"))
            }),
        }
    };
    let (year, month, day) = (field("year", None)?, field("month", None)?, field("day", None)?);
    let (hour, minute, second) = (
        field("hour", Some(12))?,
        field("min", Some(0))?,
        field("sec", Some(0))?,
    );
    let months = year * 12 + (month - 1);
    let Some(first) = i32::try_from(months.div_euclid(12))
        .ok()
        .and_then(|year| NaiveDate::from_ymd_opt(year, months.rem_euclid(12) as u32 + 1, 1))
    else {
        return Ok(None);
    };
    let Some(moment) = first
        .and_hms_opt(0, 0, 0)
        .and_then(|midnight| midnight.checked_add_signed(chrono::TimeDelta::try_days(day - 1)?))
        .and_then(|date| {
            let seconds = hour
                .checked_mul(3600)?
                .checked_add(minute.checked_mul(60)?)?
                .checked_add(second)?;
            date.checked_add_signed(chrono::TimeDelta::try_seconds(seconds)?)
        })
    else {
        return Ok(None);
    };
    Ok(Local
        .from_local_datetime(&moment)
        .earliest()
        .map(|local| local.timestamp()))
}

/// `os.date(format, time)`: `*t` gives a table, `!` asks for UTC, anything else is strftime.
fn date(lua: &Lua, format: &str, time: Option<f64>) -> mlua::Result<Value> {
    let seconds = time.map_or_else(|| Utc::now().timestamp(), |time| time.floor() as i64);
    let (utc, format) = match format.strip_prefix('!') {
        Some(rest) => (true, rest),
        None => (false, format),
    };
    let Some(moment) = DateTime::<Utc>::from_timestamp(seconds, 0) else {
        return Ok(Value::Nil);
    };
    let local = moment.with_timezone(&Local);
    if format.starts_with("*t") {
        let table = lua.create_table()?;
        let fill = |table: &Table, moment: &dyn DateParts| -> mlua::Result<()> {
            table.set("year", moment.year())?;
            table.set("month", moment.month())?;
            table.set("day", moment.day())?;
            table.set("hour", moment.hour())?;
            table.set("min", moment.minute())?;
            table.set("sec", moment.second())?;
            table.set("wday", moment.weekday_from_sunday() + 1)?;
            table.set("yday", moment.ordinal())?;
            table.set("isdst", false)
        };
        if utc {
            fill(&table, &moment)?;
        } else {
            fill(&table, &local)?;
        }
        return Ok(Value::Table(table));
    }
    let items: Vec<chrono::format::Item<'_>> = chrono::format::StrftimeItems::new(format).collect();
    if items
        .iter()
        .any(|item| matches!(item, chrono::format::Item::Error))
    {
        return Err(mlua::Error::runtime(format!(
            "bad argument #1 to 'date' (invalid conversion specifier '{format}')"
        )));
    }
    let text = if utc {
        moment.format_with_items(items.into_iter()).to_string()
    } else {
        local.format_with_items(items.into_iter()).to_string()
    };
    Ok(Value::String(lua.create_string(text)?))
}

/// What `os.date("*t")` reads from a moment, in UTC or local time.
trait DateParts {
    fn year(&self) -> i32;
    fn month(&self) -> u32;
    fn day(&self) -> u32;
    fn hour(&self) -> u32;
    fn minute(&self) -> u32;
    fn second(&self) -> u32;
    fn weekday_from_sunday(&self) -> u32;
    fn ordinal(&self) -> u32;
}

impl<Zone: TimeZone> DateParts for DateTime<Zone> {
    fn year(&self) -> i32 {
        Datelike::year(self)
    }
    fn month(&self) -> u32 {
        Datelike::month(self)
    }
    fn day(&self) -> u32 {
        Datelike::day(self)
    }
    fn hour(&self) -> u32 {
        Timelike::hour(self)
    }
    fn minute(&self) -> u32 {
        Timelike::minute(self)
    }
    fn second(&self) -> u32 {
        Timelike::second(self)
    }
    fn weekday_from_sunday(&self) -> u32 {
        Datelike::weekday(self).num_days_from_sunday()
    }
    fn ordinal(&self) -> u32 {
        Datelike::ordinal(self)
    }
}

/// Gives a loaded dkjson library a native `decode`: dkjson's own reads big API answers slowly and
/// with many times their size in memory. Its results are the same (objects and arrays carry
/// dkjson's `__jsontype` metatables, `null` becomes `nullval`); anything it can't read, or a call
/// with a start position, goes to dkjson's own decoder, so errors read as they always did.
pub fn speed_up_dkjson(lua: &Lua, library: &Value) -> mlua::Result<()> {
    let Value::Table(json) = library else {
        return Ok(());
    };
    let version: Option<String> = json.get("version").ok().flatten();
    let Some(original) = json.get::<Option<Function>>("decode").ok().flatten() else {
        return Ok(());
    };
    if !version.is_some_and(|version| version.starts_with("dkjson")) {
        return Ok(());
    }
    let decode = lua.create_function(move |lua, args: MultiValue| {
        let values: Vec<Value> = args.iter().cloned().collect();
        let text = match values.first() {
            Some(Value::String(text)) => text.clone(),
            _ => return original.call::<MultiValue>(args),
        };
        let from_start = match values.get(1) {
            None | Some(Value::Nil | Value::Integer(1)) => true,
            Some(Value::Number(position)) => *position == 1.0,
            Some(_) => false,
        };
        if !from_start {
            return original.call::<MultiValue>(args);
        }
        let bytes = text.as_bytes();
        let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return original.call::<MultiValue>(args);
        };
        let null = values.get(2).cloned().unwrap_or(Value::Nil);
        // dkjson's own metatables unless the caller passes some (even nil ones).
        let (object_meta, array_meta) = match values.get(3) {
            Some(object) => (object.clone(), values.get(4).cloned().unwrap_or(Value::Nil)),
            None => {
                let object = lua.create_table()?;
                object.set("__jsontype", "object")?;
                let array = lua.create_table()?;
                array.set("__jsontype", "array")?;
                (Value::Table(object), Value::Table(array))
            }
        };
        let value = to_lua(lua, &parsed, &null, &object_meta, &array_meta)?;
        let end = bytes.len() + 1;
        Ok(MultiValue::from_iter([value, Value::Integer(end as i64)]))
    })?;
    json.set("decode", decode)
}

fn to_lua(
    lua: &Lua,
    value: &serde_json::Value,
    null: &Value,
    object_meta: &Value,
    array_meta: &Value,
) -> mlua::Result<Value> {
    let meta = |table: &Table, meta: &Value| -> mlua::Result<()> {
        if let Value::Table(meta) = meta {
            table.set_metatable(Some(meta.clone()));
        }
        Ok(())
    };
    Ok(match value {
        serde_json::Value::Null => null.clone(),
        serde_json::Value::Bool(flag) => Value::Boolean(*flag),
        serde_json::Value::Number(number) => Value::Number(number.as_f64().unwrap_or_default()),
        serde_json::Value::String(text) => Value::String(lua.create_string(text)?),
        serde_json::Value::Array(items) => {
            let table = lua.create_table_with_capacity(items.len(), 0)?;
            for (index, item) in items.iter().enumerate() {
                let item = to_lua(lua, item, null, object_meta, array_meta)?;
                if !item.is_nil() {
                    table.raw_set(index + 1, item)?;
                }
            }
            meta(&table, array_meta)?;
            Value::Table(table)
        }
        serde_json::Value::Object(fields) => {
            let table = lua.create_table_with_capacity(0, fields.len())?;
            for (key, item) in fields {
                let item = to_lua(lua, item, null, object_meta, array_meta)?;
                if !item.is_nil() {
                    table.raw_set(key.as_str(), item)?;
                }
            }
            meta(&table, object_meta)?;
            Value::Table(table)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lua() -> Lua {
        let lua = Lua::new();
        install_os(&lua, &lua.globals()).unwrap();
        lua
    }

    #[test]
    fn os_date_and_time_follow_lua() {
        let lua = lua();
        let (year, text): (i64, String) = lua
            .load(r#"return os.date("!*t", 86400 * 365).year, os.date("!%Y-%m-%d %H:%M:%S", 1700000000)"#)
            .eval()
            .unwrap();
        assert_eq!(year, 1971);
        assert_eq!(text, "2023-11-14 22:13:20");
        // os.time and os.date("*t") agree in local time, overflowing fields included.
        let same: bool = lua
            .load(
                "local t = os.time{year=2024, month=13, day=1, hour=0}\n\
                 local d = os.date('*t', t)\n\
                 return d.year == 2025 and d.month == 1 and d.day == 1 and d.hour == 0",
            )
            .eval()
            .unwrap();
        assert!(same);
        assert!(lua.load("return os.date('%Q')").exec().is_err());
        let gap: f64 = lua.load("return os.difftime(10, 4)").eval().unwrap();
        assert_eq!(gap, 6.0);
    }

    #[test]
    fn dkjson_decodes_natively_with_its_shapes() {
        let lua = Lua::new();
        // dkjson's shape: a version and a Lua decoder, kept for what the native one won't read.
        let library: Value = lua
            .load(
                r#"local json = { version = "dkjson 2.5", null = setmetatable({}, {}) }
                function json.decode(text) return "lua decoder", 0, "fallback" end
                return json"#,
            )
            .eval()
            .unwrap();
        speed_up_dkjson(&lua, &library).unwrap();
        lua.globals().set("json", library).unwrap();
        let checks: (String, f64, bool, String, bool, String) = lua
            .load(
                r#"local data, pos = json.decode('{"a": [1, null, {"b": "ü"}], "n": null, "t": true}')
                local list = data.a
                return getmetatable(data).__jsontype, list[1], list[2] == nil, list[3].b,
                    data.n == nil and data.t, getmetatable(list).__jsontype"#,
            )
            .eval()
            .unwrap();
        assert_eq!(
            checks,
            ("object".into(), 1.0, true, "ü".into(), true, "array".into())
        );
        let null_kept: bool = lua
            .load(r#"return json.decode('[null]', 1, json.null)[1] == json.null"#)
            .eval()
            .unwrap();
        assert!(null_kept);
        // Text the native decoder can't read goes to dkjson's own.
        let fallback: String = lua.load("return json.decode('{bad')").eval().unwrap();
        assert_eq!(fallback, "lua decoder");
    }

    #[test]
    fn compiled_chunks_are_reused() {
        let lua = Lua::new();
        let code = "return 40 + 2";
        let first: i64 = load_chunk(&lua, code, "lib(answer)").unwrap().call(()).unwrap();
        let second: i64 = load_chunk(&lua, code, "lib(answer)").unwrap().call(()).unwrap();
        assert_eq!((first, second), (42, 42));
        let other = Lua::new();
        let third: i64 = load_chunk(&other, code, "lib(answer)").unwrap().call(()).unwrap();
        assert_eq!(third, 42);
    }
}
