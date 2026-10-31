//! Per-extension key-value storage (`raygun.storage`): a JSON file outside the code folder.

use std::collections::BTreeMap;
use std::path::PathBuf;

pub const STORAGE_LIMIT_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct KvStore {
    path: Option<PathBuf>,
    values: BTreeMap<String, serde_json::Value>,
}

impl KvStore {
    /// Opens the store at `path`, or an in-memory one when `None`.
    pub fn open(path: Option<PathBuf>) -> Self {
        let values = path
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| match serde_json::from_slice(&bytes) {
                Ok(values) => Some(values),
                Err(error) => {
                    tracing::warn!(%error, "extension storage unreadable, starting empty");
                    None
                }
            })
            .unwrap_or_default();
        Self { path, values }
    }

    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.values.get(key)
    }

    pub fn keys(&self) -> Vec<String> {
        self.values.keys().cloned().collect()
    }

    pub fn set(&mut self, key: &str, value: serde_json::Value) -> Result<(), String> {
        let previous = self.values.insert(key.to_owned(), value);
        self.save().inspect_err(|_| {
            match previous {
                Some(previous) => self.values.insert(key.to_owned(), previous),
                None => self.values.remove(key),
            };
        })
    }

    pub fn remove(&mut self, key: &str) -> Result<(), String> {
        if self.values.remove(key).is_some() {
            self.save()?;
        }
        Ok(())
    }

    fn save(&self) -> Result<(), String> {
        let bytes = serde_json::to_vec(&self.values).map_err(|error| error.to_string())?;
        if bytes.len() > STORAGE_LIMIT_BYTES {
            return Err(format!("storage is limited to {STORAGE_LIMIT_BYTES} bytes"));
        }
        let Some(path) = &self.path else {
            return Ok(());
        };
        let write = || -> std::io::Result<()> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let temp = path.with_extension("json.tmp");
            std::fs::write(&temp, &bytes)?;
            std::fs::rename(&temp, path)
        };
        write().map_err(|error| format!("could not save storage: {error}"))
    }
}
