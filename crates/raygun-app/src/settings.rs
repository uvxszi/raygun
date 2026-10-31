//! Typed settings over the per-key `setting` table.
//!
//! A stored row is the JSON value of one [`SettingChange`] variant, so loading reuses the same
//! deserializer that validates UI input. Rows that no longer parse (an old version's key, a value
//! edited by hand) are skipped with a warning and the default is used instead.

use raygun_db::{Db, DbError, settings};
use raygun_model::{SettingChange, Settings};

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("{0}")]
    Invalid(String),
}

pub async fn load(db: &Db) -> Result<Settings, SettingsError> {
    let mut settings = Settings::default();
    for (key, value) in settings::load(db).await? {
        match parse_row(&key, &value) {
            Ok(change) => settings.apply(change),
            Err(error) => tracing::warn!(%key, %value, %error, "ignoring unreadable setting"),
        }
    }
    Ok(settings)
}

pub async fn update(db: &Db, change: SettingChange) -> Result<Settings, SettingsError> {
    change.validate().map_err(SettingsError::Invalid)?;
    let value = match serde_json::to_value(&change)
        .map_err(|error| SettingsError::Invalid(error.to_string()))?
    {
        serde_json::Value::Object(mut fields) => fields.remove("value").unwrap_or_default(),
        _ => unreachable!("SettingChange serializes as an object"),
    };
    settings::put(db, change.key(), &value.to_string()).await?;
    load(db).await
}

fn parse_row(key: &str, value: &str) -> Result<SettingChange, String> {
    let value: serde_json::Value =
        serde_json::from_str(value).map_err(|error| error.to_string())?;
    let change: SettingChange =
        serde_json::from_value(serde_json::json!({ "key": key, "value": value }))
            .map_err(|error| error.to_string())?;
    change.validate()?;
    Ok(change)
}

#[cfg(test)]
mod tests {
    use raygun_model::Theme;
    use tempfile::TempDir;

    use super::*;

    async fn open(temp: &TempDir) -> Db {
        Db::open(&temp.path().join("raygun.db")).await.unwrap()
    }

    #[tokio::test]
    async fn defaults_when_nothing_is_stored() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;

        assert_eq!(load(&db).await.unwrap(), Settings::default());
    }

    #[tokio::test]
    async fn updates_persist_across_reopen() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;

        let updated = update(&db, SettingChange::Theme(Theme::Amoled))
            .await
            .unwrap();
        update(&db, SettingChange::Language("pt-BR".into()))
            .await
            .unwrap();
        db.close().await;
        let reopened = open(&temp).await;

        assert_eq!(updated.theme, Theme::Amoled);
        assert_eq!(
            load(&reopened).await.unwrap(),
            Settings {
                theme: Theme::Amoled,
                language: "pt-BR".into()
            }
        );
    }

    #[tokio::test]
    async fn rejects_invalid_values() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;

        let result = update(&db, SettingChange::Language("not a tag!".into())).await;

        assert!(matches!(result, Err(SettingsError::Invalid(_))));
        assert_eq!(load(&db).await.unwrap(), Settings::default());
    }

    #[tokio::test]
    async fn unreadable_rows_fall_back_to_defaults() {
        let temp = TempDir::new().unwrap();
        let db = open(&temp).await;
        settings::put(&db, "theme", "\"neon\"").await.unwrap();
        settings::put(&db, "removedInOldVersion", "true")
            .await
            .unwrap();
        settings::put(&db, "language", "\"ja\"").await.unwrap();

        let loaded = load(&db).await.unwrap();

        assert_eq!(loaded.theme, Theme::System);
        assert_eq!(loaded.language, "ja");
    }
}
