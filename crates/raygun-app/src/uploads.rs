//! Files picked in the webview, sent to Rust in base64 chunks (M46). A comic archive can be far
//! larger than one IPC message should carry, so the UI opens an upload, appends slices of the file
//! and then hands the finished file to whatever imports it.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use base64::Engine;
use uuid::Uuid;

use crate::catalog::to_message;

/// The largest file an upload may grow to.
pub const MAX_UPLOAD_BYTES: u64 = 1024 * 1024 * 1024;

pub struct Uploads {
    dir: PathBuf,
    /// Open uploads: their file and size so far.
    open: Mutex<HashMap<String, (PathBuf, u64)>>,
}

impl Uploads {
    /// Uploads are written to `dir` (a folder of their own under the downloads folder); any left
    /// from an earlier run are deleted.
    pub fn new(dir: PathBuf) -> Self {
        let _ = std::fs::remove_dir_all(&dir);
        Self {
            dir,
            open: Mutex::new(HashMap::new()),
        }
    }

    /// Starts an upload; returns its id.
    pub fn begin(&self) -> Result<String, String> {
        std::fs::create_dir_all(&self.dir).map_err(to_message)?;
        let id = Uuid::now_v7().to_string();
        let path = self.dir.join(format!("{id}.part"));
        std::fs::File::create(&path).map_err(to_message)?;
        self.open
            .lock()
            .expect("uploads poisoned")
            .insert(id.clone(), (path, 0));
        Ok(id)
    }

    /// Appends a base64 slice; returns the size so far.
    pub fn append(&self, id: &str, chunk: &str) -> Result<u64, String> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(chunk.trim())
            .map_err(|_| "The file couldn't be read".to_owned())?;
        let mut open = self.open.lock().expect("uploads poisoned");
        let (path, size) = open
            .get_mut(id)
            .ok_or_else(|| "This upload isn't open".to_owned())?;
        let grown = *size + bytes.len() as u64;
        if grown > MAX_UPLOAD_BYTES {
            let path = path.clone();
            open.remove(id);
            let _ = std::fs::remove_file(path);
            return Err("This file is larger than 1 GB".into());
        }
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&*path)
            .map_err(to_message)?;
        file.write_all(&bytes).map_err(to_message)?;
        *size = grown;
        Ok(grown)
    }

    /// Closes an upload and hands over its file; the caller deletes it when done.
    pub fn take(&self, id: &str) -> Result<PathBuf, String> {
        self.open
            .lock()
            .expect("uploads poisoned")
            .remove(id)
            .map(|(path, _)| path)
            .ok_or_else(|| "This upload isn't open".to_owned())
    }

    /// Drops an upload that won't be finished.
    pub fn cancel(&self, id: &str) {
        if let Ok(path) = self.take(id) {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_append_into_one_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let uploads = Uploads::new(temp.path().join("uploads"));
        let id = uploads.begin().unwrap();
        let engine = base64::engine::general_purpose::STANDARD;
        assert_eq!(uploads.append(&id, &engine.encode(b"hello ")).unwrap(), 6);
        assert_eq!(uploads.append(&id, &engine.encode(b"world")).unwrap(), 11);
        let path = uploads.take(&id).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello world");
        assert!(uploads.append(&id, &engine.encode(b"!")).is_err(), "closed");
        assert!(uploads.append("nope", "aGk=").is_err());
    }
}
