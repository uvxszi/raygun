//! Encrypted API keys and tokens (`secrets.db`): a JSON map sealed with ChaCha20-Poly1305.
//!
//! The file is `RGS1`, a random 12-byte nonce, then the ciphertext. The caller keeps the key
//! (device-local, never synced); a file the key can't open is treated as empty.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

const MAGIC: &[u8; 4] = b"RGS1";
const NONCE_LEN: usize = 12;
pub const KEY_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("couldn't save secrets: {0}")]
    Io(#[from] std::io::Error),
    #[error("couldn't encrypt secrets")]
    Crypto,
    #[error("no random numbers available: {0}")]
    Random(String),
}

/// A fresh random key for a new vault.
pub fn new_key() -> Result<[u8; KEY_LEN], SecretsError> {
    let mut key = [0u8; KEY_LEN];
    getrandom::fill(&mut key).map_err(|error| SecretsError::Random(error.to_string()))?;
    Ok(key)
}

pub struct Vault {
    path: PathBuf,
    cipher: ChaCha20Poly1305,
    values: Mutex<BTreeMap<String, String>>,
}

impl Vault {
    /// Opens the vault at `path` (missing is fine). A file this key can't open starts empty and
    /// is replaced on the next save.
    pub fn open(path: &Path, key: &[u8; KEY_LEN]) -> Self {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
        let values = match std::fs::read(path) {
            Ok(bytes) => decrypt(&cipher, &bytes).unwrap_or_else(|| {
                tracing::warn!(path = %path.display(), "secrets file unreadable; starting empty");
                BTreeMap::new()
            }),
            Err(_) => BTreeMap::new(),
        };
        Self {
            path: path.to_owned(),
            cipher,
            values: Mutex::new(values),
        }
    }

    pub fn get(&self, name: &str) -> Option<String> {
        self.lock().get(name).cloned()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.lock().contains_key(name)
    }

    /// The last four characters of a stored value, for showing which key is saved.
    pub fn hint(&self, name: &str) -> Option<String> {
        let values = self.lock();
        let value = values.get(name)?;
        let count = value.chars().count();
        Some(value.chars().skip(count.saturating_sub(4)).collect())
    }

    /// Stores a value (`None` removes it) and saves the file.
    pub fn set(&self, name: &str, value: Option<&str>) -> Result<(), SecretsError> {
        let mut values = self.lock();
        match value {
            Some(value) => values.insert(name.to_owned(), value.to_owned()),
            None => values.remove(name),
        };
        self.save(&values)
    }

    fn save(&self, values: &BTreeMap<String, String>) -> Result<(), SecretsError> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).map_err(|error| SecretsError::Random(error.to_string()))?;
        let plain = serde_json::to_vec(values).map_err(|_| SecretsError::Crypto)?;
        let sealed = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plain.as_slice())
            .map_err(|_| SecretsError::Crypto)?;
        let mut bytes = Vec::with_capacity(MAGIC.len() + NONCE_LEN + sealed.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&nonce);
        bytes.extend_from_slice(&sealed);
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let partial = self.path.with_extension("db.part");
        std::fs::write(&partial, &bytes)?;
        std::fs::rename(&partial, &self.path)?;
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, String>> {
        self.values
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

fn decrypt(cipher: &ChaCha20Poly1305, bytes: &[u8]) -> Option<BTreeMap<String, String>> {
    let rest = bytes.strip_prefix(MAGIC.as_slice())?;
    if rest.len() < NONCE_LEN {
        return None;
    }
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    let plain = cipher.decrypt(Nonce::from_slice(nonce), sealed).ok()?;
    serde_json::from_slice(&plain).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_encrypted_and_wrong_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.db");
        let key = new_key().unwrap();
        let vault = Vault::open(&path, &key);
        assert_eq!(vault.get("provider:a"), None);
        vault.set("provider:a", Some("sk-ant-secret-1234")).unwrap();
        vault.set("provider:b", Some("abc")).unwrap();
        assert_eq!(vault.hint("provider:a").as_deref(), Some("1234"));
        assert_eq!(vault.hint("provider:b").as_deref(), Some("abc"));

        let raw = std::fs::read(&path).unwrap();
        assert!(raw.starts_with(MAGIC));
        let text = String::from_utf8_lossy(&raw);
        assert!(
            !text.contains("sk-ant") && !text.contains("provider"),
            "no plain text"
        );

        let reopened = Vault::open(&path, &key);
        assert_eq!(
            reopened.get("provider:a").as_deref(),
            Some("sk-ant-secret-1234")
        );
        reopened.set("provider:a", None).unwrap();
        let again = Vault::open(&path, &key);
        assert!(!again.contains("provider:a"));
        assert!(again.contains("provider:b"));

        let other = Vault::open(&path, &new_key().unwrap());
        assert_eq!(
            other.get("provider:b"),
            None,
            "a wrong key opens an empty vault"
        );
        std::fs::write(&path, b"garbage").unwrap();
        assert_eq!(Vault::open(&path, &key).get("provider:b"), None);
    }
}
