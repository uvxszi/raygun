//! What was taken out of Home's Continue rows (Continue reading, Continue listening) without
//! losing its place: each by its key (`reading:<title id>`, `listening:<episode id>`, a book's
//! `listening:<podcast id>`) with when. It comes back once it's read or listened to after that.
//! Device-local (`local_meta`), as Home's rows are each device's own.

use std::collections::HashMap;

use crate::now_ms;

/// Where the taken-out keys are kept: JSON `{ key: unix ms }`.
const STORED: &str = "home.continueHidden";
/// Marks older than this are dropped when others are added (a title untouched for a year).
const KEEP_MS: i64 = 365 * 24 * 60 * 60 * 1000;

/// A title's key in Continue reading.
pub fn reading_key(title_id: &str) -> String {
    format!("reading:{title_id}")
}

/// An episode's (or a book's, by its podcast id) key in Continue listening.
pub fn listening_key(id: &str) -> String {
    format!("listening:{id}")
}

/// The taken-out keys with when.
pub async fn hidden(db: &raygun_db::Db) -> HashMap<String, i64> {
    match raygun_db::local_value(db, STORED).await {
        Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_default(),
        Ok(None) => HashMap::new(),
        Err(error) => {
            tracing::warn!(%error, "couldn't read what Continue leaves out");
            HashMap::new()
        }
    }
}

/// Whether an item shows in Continue: not taken out, or used (`last_ms`) since it was.
pub fn shows(hidden: &HashMap<String, i64>, key: &str, last_ms: Option<f64>) -> bool {
    match hidden.get(key) {
        None => true,
        Some(at) => last_ms.is_some_and(|last| last > *at as f64),
    }
}

/// `hidden` with `keys` taken out at `now`, marks older than a year dropped.
fn with_hidden(
    mut hidden: HashMap<String, i64>,
    keys: &[String],
    now: i64,
) -> HashMap<String, i64> {
    hidden.retain(|_, at| now - *at < KEEP_MS);
    for key in keys {
        hidden.insert(key.clone(), now);
    }
    hidden
}

/// Takes `keys` out of Continue until they're used again; how many.
pub async fn hide(db: &raygun_db::Db, keys: &[String]) -> Result<u32, String> {
    let next = with_hidden(hidden(db).await, keys, now_ms());
    let json = serde_json::to_string(&next).map_err(|error| error.to_string())?;
    raygun_db::set_local_value(db, STORED, &json)
        .await
        .map_err(|error| error.to_string())?;
    Ok(u32::try_from(keys.len()).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taken_out_until_used_again() {
        let hidden = with_hidden(HashMap::new(), &[reading_key("t1")], 1_000);
        assert!(!shows(&hidden, "reading:t1", Some(900.0)));
        assert!(!shows(&hidden, "reading:t1", None));
        // Read again afterwards: back in Continue.
        assert!(shows(&hidden, "reading:t1", Some(1_500.0)));
        assert!(shows(&hidden, &listening_key("e1"), Some(10.0)));
    }

    #[test]
    fn old_marks_go_when_others_are_added() {
        let hidden = with_hidden(HashMap::new(), &["reading:old".to_owned()], 0);
        let later = with_hidden(hidden, &["reading:new".to_owned()], KEEP_MS + 1);
        assert!(!later.contains_key("reading:old"));
        assert_eq!(later.get("reading:new"), Some(&(KEEP_MS + 1)));
    }
}
