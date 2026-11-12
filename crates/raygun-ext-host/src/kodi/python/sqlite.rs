//! SQLite for Kodi add-ons' `sqlite3` (`lib/sqlite3`), over the library the app already links.
//! One statement at a time, its rows read at once; files only in the add-on's own folders.

use std::ffi::{CStr, CString, c_int};
use std::path::Path;
use std::ptr;

use libsqlite3_sys as ffi;

/// A value going in or coming out.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

/// What a statement gave: its columns (name, declared type), its rows, the rows it changed and
/// the last row id inserted.
#[derive(Debug, Default)]
pub(crate) struct Outcome {
    pub columns: Vec<(String, Option<String>)>,
    pub rows: Vec<Vec<Value>>,
    pub changes: i64,
    pub last_row: i64,
}

/// SQLite's code and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SqlError {
    pub code: i32,
    pub message: String,
}

/// Bound parameters: by position, or by name (`:name`).
pub(crate) enum Parameters {
    Positional(Vec<Value>),
    Named(Vec<(String, Value)>),
}

/// Most rows one statement may give.
const MAX_ROWS: usize = 200_000;

pub(crate) struct Database {
    handle: *mut ffi::sqlite3,
}

// SAFETY: SQLite is built threadsafe (SQLITE_THREADSAFE=1 in libsqlite3-sys's bundle) and a
// `Database` is only used behind the interpreter's lock, one call at a time.
unsafe impl Send for Database {}

fn error_of(handle: *mut ffi::sqlite3, code: c_int) -> SqlError {
    let message = if handle.is_null() {
        // SAFETY: sqlite3_errstr returns a static string for any code.
        unsafe { CStr::from_ptr(ffi::sqlite3_errstr(code)) }
            .to_string_lossy()
            .into_owned()
    } else {
        // SAFETY: the handle is open; the message lives until the next call on it.
        unsafe { CStr::from_ptr(ffi::sqlite3_errmsg(handle)) }
            .to_string_lossy()
            .into_owned()
    };
    SqlError {
        code: if handle.is_null() {
            code
        } else {
            // SAFETY: the handle is open.
            unsafe { ffi::sqlite3_extended_errcode(handle) }
        },
        message,
    }
}

impl Database {
    /// Opens (or creates) a database file, or one in memory for `None`.
    pub fn open(path: Option<&Path>, busy_ms: i32) -> Result<Self, SqlError> {
        let name = match path {
            Some(path) => {
                CString::new(path.to_string_lossy().as_bytes()).map_err(|_| SqlError {
                    code: ffi::SQLITE_CANTOPEN,
                    message: "unable to open database file".into(),
                })?
            }
            None => CString::new(":memory:").expect("no NUL"),
        };
        let mut handle = ptr::null_mut();
        let flags = ffi::SQLITE_OPEN_READWRITE
            | ffi::SQLITE_OPEN_CREATE
            | ffi::SQLITE_OPEN_FULLMUTEX
            | ffi::SQLITE_OPEN_NOFOLLOW;
        // SAFETY: valid C string and out pointer; a failed open still needs closing.
        let code = unsafe { ffi::sqlite3_open_v2(name.as_ptr(), &mut handle, flags, ptr::null()) };
        if code != ffi::SQLITE_OK {
            let error = error_of(handle, code);
            // SAFETY: closing a handle sqlite3_open_v2 gave (null is fine).
            unsafe { ffi::sqlite3_close(handle) };
            return Err(error);
        }
        // SAFETY: open handle. Extensions stay off; attaching other files is refused below.
        unsafe {
            ffi::sqlite3_busy_timeout(handle, busy_ms.max(0));
            ffi::sqlite3_extended_result_codes(handle, 1);
            ffi::sqlite3_limit(handle, ffi::SQLITE_LIMIT_ATTACHED, 0);
            ffi::sqlite3_limit(handle, ffi::SQLITE_LIMIT_LENGTH, 64 * 1024 * 1024);
        }
        Ok(Self { handle })
    }

    pub fn autocommit(&self) -> bool {
        // SAFETY: open handle.
        unsafe { ffi::sqlite3_get_autocommit(self.handle) != 0 }
    }

    pub fn total_changes(&self) -> i64 {
        // SAFETY: open handle.
        i64::from(unsafe { ffi::sqlite3_total_changes(self.handle) })
    }

    fn bind(
        &self,
        statement: *mut ffi::sqlite3_stmt,
        index: c_int,
        value: &Value,
    ) -> Result<(), SqlError> {
        // SAFETY: a prepared statement of this handle; SQLITE_TRANSIENT copies the bytes.
        let code = unsafe {
            match value {
                Value::Null => ffi::sqlite3_bind_null(statement, index),
                Value::Integer(number) => ffi::sqlite3_bind_int64(statement, index, *number),
                Value::Real(number) => ffi::sqlite3_bind_double(statement, index, *number),
                Value::Text(text) => ffi::sqlite3_bind_text64(
                    statement,
                    index,
                    text.as_ptr().cast(),
                    text.len() as u64,
                    ffi::SQLITE_TRANSIENT(),
                    ffi::SQLITE_UTF8 as u8,
                ),
                Value::Blob(bytes) => ffi::sqlite3_bind_blob64(
                    statement,
                    index,
                    bytes.as_ptr().cast(),
                    bytes.len() as u64,
                    ffi::SQLITE_TRANSIENT(),
                ),
            }
        };
        if code == ffi::SQLITE_OK {
            Ok(())
        } else {
            Err(error_of(self.handle, code))
        }
    }

    /// Runs one statement (anything after it is refused, as Python's `sqlite3` does).
    pub fn execute(&self, sql: &str, parameters: &Parameters) -> Result<Outcome, SqlError> {
        refuse_other_files(sql)?;
        let text = CString::new(sql).map_err(|_| SqlError {
            code: ffi::SQLITE_MISUSE,
            message: "the statement contains a null character".into(),
        })?;
        let mut statement = ptr::null_mut();
        let mut tail = ptr::null();
        // SAFETY: valid handle and C string; SQLite sets `tail` within `text`.
        let code = unsafe {
            ffi::sqlite3_prepare_v2(self.handle, text.as_ptr(), -1, &mut statement, &mut tail)
        };
        if code != ffi::SQLITE_OK {
            return Err(error_of(self.handle, code));
        }
        let finish = Finalize(statement);
        if !tail.is_null() {
            // SAFETY: `tail` points into `text`, NUL-terminated.
            let rest = unsafe { CStr::from_ptr(tail) }.to_string_lossy();
            let rest = rest.trim();
            if !rest.is_empty() && !rest.chars().all(|c| c == ';' || c.is_whitespace()) {
                return Err(SqlError {
                    code: ffi::SQLITE_MISUSE,
                    message: "You can only execute one statement at a time.".into(),
                });
            }
        }
        if statement.is_null() {
            return Ok(Outcome::default());
        }
        // SAFETY: prepared statement.
        let wanted = unsafe { ffi::sqlite3_bind_parameter_count(statement) };
        match parameters {
            Parameters::Positional(values) => {
                if values.len() as c_int != wanted {
                    return Err(SqlError {
                        code: ffi::SQLITE_RANGE,
                        message: format!(
                            "Incorrect number of bindings supplied. The current statement uses {wanted}, and there are {} supplied.",
                            values.len()
                        ),
                    });
                }
                for (index, value) in values.iter().enumerate() {
                    self.bind(statement, index as c_int + 1, value)?;
                }
            }
            Parameters::Named(values) => {
                for index in 1..=wanted {
                    // SAFETY: prepared statement, index in range.
                    let name = unsafe { ffi::sqlite3_bind_parameter_name(statement, index) };
                    if name.is_null() {
                        continue;
                    }
                    // SAFETY: SQLite's own NUL-terminated name.
                    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
                    let key = name.trim_start_matches([':', '@', '$']);
                    let value = values
                        .iter()
                        .find(|(given, _)| given == key)
                        .map(|(_, value)| value)
                        .ok_or_else(|| SqlError {
                            code: ffi::SQLITE_RANGE,
                            message: format!(
                                "You did not supply a value for binding parameter :{key}."
                            ),
                        })?;
                    self.bind(statement, index, value)?;
                }
            }
        }
        // SAFETY: prepared statement.
        let count = unsafe { ffi::sqlite3_column_count(statement) };
        let columns = (0..count)
            .map(|index| {
                // SAFETY: column index in range; the strings are SQLite's.
                let name = unsafe { ffi::sqlite3_column_name(statement, index) };
                let declared = unsafe { ffi::sqlite3_column_decltype(statement, index) };
                let name = if name.is_null() {
                    String::new()
                } else {
                    unsafe { CStr::from_ptr(name) }
                        .to_string_lossy()
                        .into_owned()
                };
                let declared = (!declared.is_null()).then(|| {
                    unsafe { CStr::from_ptr(declared) }
                        .to_string_lossy()
                        .into_owned()
                });
                (name, declared)
            })
            .collect();
        let mut rows = Vec::new();
        loop {
            // SAFETY: prepared statement.
            let code = unsafe { ffi::sqlite3_step(statement) };
            match code {
                ffi::SQLITE_ROW => {
                    if rows.len() >= MAX_ROWS {
                        return Err(SqlError {
                            code: ffi::SQLITE_TOOBIG,
                            message: "the query gave too many rows".into(),
                        });
                    }
                    rows.push((0..count).map(|index| column(statement, index)).collect());
                }
                ffi::SQLITE_DONE => break,
                code => return Err(error_of(self.handle, code)),
            }
        }
        drop(finish);
        // SAFETY: open handle.
        let (changes, last_row) = unsafe {
            (
                i64::from(ffi::sqlite3_changes(self.handle)),
                ffi::sqlite3_last_insert_rowid(self.handle),
            )
        };
        Ok(Outcome {
            columns,
            rows,
            changes,
            last_row,
        })
    }

    /// Runs several statements (`executescript`).
    pub fn script(&self, sql: &str) -> Result<(), SqlError> {
        refuse_other_files(sql)?;
        let text = CString::new(sql).map_err(|_| SqlError {
            code: ffi::SQLITE_MISUSE,
            message: "the script contains a null character".into(),
        })?;
        let mut message = ptr::null_mut();
        // SAFETY: valid handle and C string; SQLite allocates the message, freed below.
        let code = unsafe {
            ffi::sqlite3_exec(
                self.handle,
                text.as_ptr(),
                None,
                ptr::null_mut(),
                &mut message,
            )
        };
        if code == ffi::SQLITE_OK {
            return Ok(());
        }
        let text = if message.is_null() {
            error_of(self.handle, code).message
        } else {
            // SAFETY: SQLite's message, freed with sqlite3_free.
            let text = unsafe { CStr::from_ptr(message) }
                .to_string_lossy()
                .into_owned();
            unsafe { ffi::sqlite3_free(message.cast()) };
            text
        };
        Err(SqlError {
            code,
            message: text,
        })
    }
}

/// `VACUUM INTO` writes a copy wherever it's told: only the add-on's own files are its.
fn refuse_other_files(sql: &str) -> Result<(), SqlError> {
    let lower = sql.to_ascii_lowercase();
    if let Some(at) = lower.find("vacuum")
        && lower[at..].contains("into")
    {
        return Err(SqlError {
            code: ffi::SQLITE_AUTH,
            message: "VACUUM INTO isn't allowed".into(),
        });
    }
    Ok(())
}

fn column(statement: *mut ffi::sqlite3_stmt, index: c_int) -> Value {
    // SAFETY: a statement on a row, index in range; pointers read before the next step.
    unsafe {
        match ffi::sqlite3_column_type(statement, index) {
            ffi::SQLITE_INTEGER => Value::Integer(ffi::sqlite3_column_int64(statement, index)),
            ffi::SQLITE_FLOAT => Value::Real(ffi::sqlite3_column_double(statement, index)),
            ffi::SQLITE_TEXT => {
                let text = ffi::sqlite3_column_text(statement, index);
                let length = ffi::sqlite3_column_bytes(statement, index).max(0) as usize;
                if text.is_null() {
                    Value::Text(String::new())
                } else {
                    let bytes = std::slice::from_raw_parts(text, length);
                    Value::Text(String::from_utf8_lossy(bytes).into_owned())
                }
            }
            ffi::SQLITE_BLOB => {
                let blob = ffi::sqlite3_column_blob(statement, index);
                let length = ffi::sqlite3_column_bytes(statement, index).max(0) as usize;
                if blob.is_null() {
                    Value::Blob(Vec::new())
                } else {
                    Value::Blob(std::slice::from_raw_parts(blob.cast::<u8>(), length).to_vec())
                }
            }
            _ => Value::Null,
        }
    }
}

struct Finalize(*mut ffi::sqlite3_stmt);

impl Drop for Finalize {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: a statement prepared on a still-open handle.
            unsafe { ffi::sqlite3_finalize(self.0) };
        }
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        // SAFETY: every statement is finalized as its call ends, so the handle closes.
        unsafe { ffi::sqlite3_close(self.handle) };
    }
}

/// SQLite's version (`3.46.0`).
pub(crate) fn version() -> String {
    // SAFETY: a static string.
    unsafe { CStr::from_ptr(ffi::sqlite3_libversion()) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_run_one_at_a_time() {
        let database = Database::open(None, 1000).unwrap();
        database
            .script("CREATE TABLE cache (key TEXT PRIMARY KEY, value BLOB, date INTEGER);")
            .unwrap();
        let inserted = database
            .execute(
                "INSERT INTO cache VALUES (?, ?, ?)",
                &Parameters::Positional(vec![
                    Value::Text("a".into()),
                    Value::Blob(vec![1, 2]),
                    Value::Integer(5),
                ]),
            )
            .unwrap();
        assert_eq!(inserted.changes, 1);
        assert_eq!(inserted.last_row, 1);
        let found = database
            .execute(
                "SELECT * FROM cache WHERE key = :key",
                &Parameters::Named(vec![("key".into(), Value::Text("a".into()))]),
            )
            .unwrap();
        assert_eq!(found.columns[0], ("key".into(), Some("TEXT".into())));
        assert_eq!(
            found.rows,
            [vec![
                Value::Text("a".into()),
                Value::Blob(vec![1, 2]),
                Value::Integer(5)
            ]]
        );
        let duplicate = database
            .execute(
                "INSERT INTO cache VALUES ('a', NULL, 0)",
                &Parameters::Positional(Vec::new()),
            )
            .unwrap_err();
        assert_eq!(duplicate.code & 0xff, ffi::SQLITE_CONSTRAINT);
        assert!(
            database
                .execute("SELECT 1; SELECT 2", &Parameters::Positional(Vec::new()))
                .is_err()
        );
        assert!(
            database
                .execute(
                    "ATTACH DATABASE '/tmp/other.db' AS other",
                    &Parameters::Positional(Vec::new())
                )
                .is_err(),
            "no other files"
        );
        assert!(!version().is_empty());
    }
}
