//! The permissions Raygun uses (Settings → Permissions) and the questions asked when a feature
//! first needs one: the first playback while Raygun is restricted in the background (battery), the
//! first download (notifications), adding a folder to Local Files (media). Android reports each
//! permission's state through the `raygun-audio` plugin; other platforms have none to ask for,
//! besides notifications, which are always allowed there.
//!
//! A question is asked once per reason: whatever the answer, Settings → Permissions is the way
//! afterwards, so nothing asks again and again.

use std::collections::BTreeMap;
use std::sync::Mutex;

use raygun_model::{
    AccessState, AppPermission, PermissionKind, PermissionPrompt, PermissionReason, PlaybackStatus,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::{PermissionPromptEvent, PlaybackStateEvent};
use crate::startup::AppDb;

/// Questions already asked, by `<reason>:<kind>`: when (unix ms).
const ASKED_KEY: &str = "permissions.asked";

/// The questions waiting to be shown, and whether this run's first playback was seen.
pub struct PermissionPrompts {
    app: AppHandle,
    pending: Mutex<Option<PermissionPrompt>>,
    played: std::sync::atomic::AtomicBool,
}

impl PermissionPrompts {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            pending: Mutex::new(None),
            played: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Follows playback for the first playback's question.
    pub fn start(app: &AppHandle) {
        let handle = app.clone();
        PlaybackStateEvent::listen(app, move |event| {
            if event.payload.0.status != PlaybackStatus::Playing {
                return;
            }
            let Some(prompts) = handle.try_state::<PermissionPrompts>() else {
                return;
            };
            if prompts
                .played
                .swap(true, std::sync::atomic::Ordering::SeqCst)
            {
                return;
            }
            let handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let prompts = handle.state::<PermissionPrompts>();
                prompts.offer(PermissionReason::Playback).await;
            });
        });
    }

    /// Asks for the permission `reason` needs, if it isn't allowed and wasn't asked before.
    pub async fn offer(&self, reason: PermissionReason) {
        let Ok(states) = list(&self.app).await else {
            return;
        };
        let Some(kind) = needed_for(reason, &states) else {
            return;
        };
        let asked = self.asked().await;
        if asked.contains_key(&asked_key(reason, kind)) {
            return;
        }
        let prompt = PermissionPrompt { kind, reason };
        *self.pending.lock().expect("prompt poisoned") = Some(prompt);
        tracing::info!(?kind, ?reason, "asking for a permission");
        let _ = PermissionPromptEvent(prompt).emit(&self.app);
    }

    /// The question waiting to be shown (raised before the UI listened), taken.
    pub fn take_pending(&self) -> Option<PermissionPrompt> {
        self.pending.lock().expect("prompt poisoned").take()
    }

    /// The answer to a question: remembered either way, and Allow asks Android.
    pub async fn answer(
        &self,
        prompt: PermissionPrompt,
        allow: bool,
    ) -> Result<Vec<AppPermission>, String> {
        {
            let mut pending = self.pending.lock().expect("prompt poisoned");
            if *pending == Some(prompt) {
                *pending = None;
            }
        }
        let mut asked = self.asked().await;
        asked.insert(
            asked_key(prompt.reason, prompt.kind),
            crate::catalog::now_ms(),
        );
        if let Ok(json) = serde_json::to_string(&asked) {
            let _ = raygun_db::set_local_value(&self.db(), ASKED_KEY, &json).await;
        }
        if allow {
            request(&self.app, prompt.kind).await
        } else {
            platform::decline(&self.app, prompt.kind).await;
            list(&self.app).await
        }
    }

    async fn asked(&self) -> BTreeMap<String, i64> {
        raygun_db::local_value(&self.db(), ASKED_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    fn db(&self) -> std::sync::Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }
}

fn asked_key(reason: PermissionReason, kind: PermissionKind) -> String {
    format!("{}:{}", reason_name(reason), kind_name(kind))
}

/// What a feature needs, when it isn't allowed and asking can change that. The first playback
/// only asks when Raygun is restricted in the background: Android's usual battery limits leave
/// playback running (its media notification keeps it going), and playback controls show without
/// the notification permission.
pub fn needed_for(reason: PermissionReason, states: &[AppPermission]) -> Option<PermissionKind> {
    let state = |kind: PermissionKind| {
        states
            .iter()
            .find(|permission| permission.kind == kind)
            .map(|permission| permission.state)
    };
    match reason {
        PermissionReason::Playback => (state(PermissionKind::Battery)
            == Some(AccessState::Restricted))
        .then_some(PermissionKind::Battery),
        PermissionReason::Download => (state(PermissionKind::Notifications)
            == Some(AccessState::Denied))
        .then_some(PermissionKind::Notifications),
        PermissionReason::Folders => matches!(
            state(PermissionKind::Media),
            Some(AccessState::Denied | AccessState::Blocked)
        )
        .then_some(PermissionKind::Media),
    }
}

/// The names the plugin knows the permissions by.
pub fn kind_name(kind: PermissionKind) -> &'static str {
    match kind {
        PermissionKind::Notifications => "notifications",
        PermissionKind::Battery => "battery",
        PermissionKind::Media => "media",
        PermissionKind::Folders => "folders",
        PermissionKind::LocalNetwork => "localNetwork",
        PermissionKind::InstallApps => "installApps",
        PermissionKind::Camera => "camera",
    }
}

fn reason_name(reason: PermissionReason) -> &'static str {
    match reason {
        PermissionReason::Playback => "playback",
        PermissionReason::Download => "download",
        PermissionReason::Folders => "folders",
    }
}

/// The plugin's answer: `[{"kind", "state", "count"}]`, rows it doesn't know left out.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn parse_states(value: &serde_json::Value) -> Vec<AppPermission> {
    let Some(rows) = value.get("permissions").and_then(|rows| rows.as_array()) else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|row| serde_json::from_value::<AppPermission>(row.clone()).ok())
        .collect()
}

/// Every permission Raygun uses on this device, and where it stands.
pub async fn list(app: &AppHandle) -> Result<Vec<AppPermission>, String> {
    platform::list(app).await
}

/// Asks for a permission: the system's question, or its settings page where only that can help.
pub async fn request(app: &AppHandle, kind: PermissionKind) -> Result<Vec<AppPermission>, String> {
    platform::request(app, kind).await
}

#[cfg(target_os = "android")]
mod platform {
    use raygun_model::{AppPermission, PermissionKind};
    use tauri::{AppHandle, Manager};
    use tauri_plugin_raygun_audio::NativeAudio;

    /// Runs a plugin call on a blocking thread: it waits for Android's main thread, and for the
    /// user while a question or a settings page shows.
    async fn call<T: Send + 'static>(
        app: &AppHandle,
        run: impl FnOnce(&NativeAudio<tauri::Wry>) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let plugin = app
                .try_state::<NativeAudio<tauri::Wry>>()
                .ok_or_else(|| "Android's permissions aren't available".to_owned())?;
            run(&plugin)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    pub async fn list(app: &AppHandle) -> Result<Vec<AppPermission>, String> {
        let value = call(app, |plugin| plugin.permission_states()).await?;
        Ok(super::parse_states(&value))
    }

    pub async fn request(
        app: &AppHandle,
        kind: PermissionKind,
    ) -> Result<Vec<AppPermission>, String> {
        let name = super::kind_name(kind);
        let value = call(app, move |plugin| plugin.request_permission(name)).await?;
        Ok(super::parse_states(&value))
    }

    pub async fn open_settings(app: &AppHandle, kind: PermissionKind) -> Result<(), String> {
        let name = super::kind_name(kind);
        call(app, move |plugin| plugin.open_permission_settings(name)).await
    }

    /// "Not now": the system isn't asked right after either (adding a folder asks for media).
    pub async fn decline(app: &AppHandle, kind: PermissionKind) {
        let name = super::kind_name(kind);
        let _ = call(app, move |plugin| plugin.decline_permission(name)).await;
    }
}

/// Desktop: notifications are the only permission, and the system allows them.
#[cfg(not(target_os = "android"))]
mod platform {
    use raygun_model::{AccessState, AppPermission, PermissionKind};
    use tauri::AppHandle;

    pub async fn list(_app: &AppHandle) -> Result<Vec<AppPermission>, String> {
        Ok(vec![AppPermission {
            kind: PermissionKind::Notifications,
            state: AccessState::Granted,
            count: None,
        }])
    }

    pub async fn request(
        app: &AppHandle,
        _kind: PermissionKind,
    ) -> Result<Vec<AppPermission>, String> {
        list(app).await
    }

    pub async fn open_settings(_app: &AppHandle, _kind: PermissionKind) -> Result<(), String> {
        Err("There's nothing to allow on this device".into())
    }

    pub async fn decline(_app: &AppHandle, _kind: PermissionKind) {}
}

// ---- Commands ----

/// Every permission Raygun uses on this device, and where it stands.
#[tauri::command]
#[specta::specta]
pub async fn permissions_list(app: AppHandle) -> Result<Vec<AppPermission>, String> {
    list(&app).await
}

/// Asks for a permission: the system's question, or its page in the system settings when only
/// that can allow it. Returns every permission's state afterwards.
#[tauri::command]
#[specta::specta]
pub async fn permission_request(
    app: AppHandle,
    kind: PermissionKind,
) -> Result<Vec<AppPermission>, String> {
    request(&app, kind).await
}

/// Opens a permission's page in the system settings.
#[tauri::command]
#[specta::specta]
pub async fn permission_open_settings(app: AppHandle, kind: PermissionKind) -> Result<(), String> {
    platform::open_settings(&app, kind).await
}

/// The answer to a question a feature asked: Allow asks the system, Not now leaves it.
#[tauri::command]
#[specta::specta]
pub async fn permission_prompt_answer(
    prompts: tauri::State<'_, PermissionPrompts>,
    prompt: PermissionPrompt,
    allow: bool,
) -> Result<Vec<AppPermission>, String> {
    prompts.answer(prompt, allow).await
}

/// A question raised before the UI was listening (the first download at start), taken.
#[tauri::command]
#[specta::specta]
pub fn permission_prompt_pending(
    prompts: tauri::State<'_, PermissionPrompts>,
) -> Option<PermissionPrompt> {
    prompts.take_pending()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permission(kind: PermissionKind, state: AccessState) -> AppPermission {
        AppPermission {
            kind,
            state,
            count: None,
        }
    }

    #[test]
    fn playback_asks_only_when_restricted() {
        let optimized = [permission(PermissionKind::Battery, AccessState::Optimized)];
        assert_eq!(needed_for(PermissionReason::Playback, &optimized), None);
        let restricted = [permission(PermissionKind::Battery, AccessState::Restricted)];
        assert_eq!(
            needed_for(PermissionReason::Playback, &restricted),
            Some(PermissionKind::Battery)
        );
        assert_eq!(needed_for(PermissionReason::Playback, &[]), None);
    }

    #[test]
    fn downloads_ask_for_notifications_android_can_still_ask_for() {
        let denied = [permission(
            PermissionKind::Notifications,
            AccessState::Denied,
        )];
        assert_eq!(
            needed_for(PermissionReason::Download, &denied),
            Some(PermissionKind::Notifications)
        );
        // Blocked: only the settings can help, and that's Settings → Permissions' job.
        let blocked = [permission(
            PermissionKind::Notifications,
            AccessState::Blocked,
        )];
        assert_eq!(needed_for(PermissionReason::Download, &blocked), None);
        let granted = [permission(
            PermissionKind::Notifications,
            AccessState::Granted,
        )];
        assert_eq!(needed_for(PermissionReason::Download, &granted), None);
    }

    #[test]
    fn folders_explain_media_access_until_it_is_allowed() {
        for state in [AccessState::Denied, AccessState::Blocked] {
            assert_eq!(
                needed_for(
                    PermissionReason::Folders,
                    &[permission(PermissionKind::Media, state)]
                ),
                Some(PermissionKind::Media)
            );
        }
        assert_eq!(
            needed_for(
                PermissionReason::Folders,
                &[permission(PermissionKind::Media, AccessState::Granted)]
            ),
            None
        );
    }

    #[test]
    fn reads_the_plugin_rows_and_skips_unknown_ones() {
        let value = serde_json::json!({
            "permissions": [
                { "kind": "notifications", "state": "denied", "count": null },
                { "kind": "battery", "state": "restricted" },
                { "kind": "folders", "state": "granted", "count": 2 },
                { "kind": "microphone", "state": "granted" },
            ]
        });
        let states = parse_states(&value);
        assert_eq!(states.len(), 3);
        assert_eq!(states[1].state, AccessState::Restricted);
        assert_eq!(states[2].count, Some(2));
        assert!(parse_states(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn plugin_names_match_the_serialized_kinds() {
        for kind in [
            PermissionKind::Notifications,
            PermissionKind::Battery,
            PermissionKind::Media,
            PermissionKind::Folders,
            PermissionKind::LocalNetwork,
            PermissionKind::InstallApps,
            PermissionKind::Camera,
        ] {
            assert_eq!(
                serde_json::to_value(kind).unwrap(),
                serde_json::Value::String(kind_name(kind).into())
            );
        }
        assert_eq!(
            asked_key(PermissionReason::Download, PermissionKind::Notifications),
            "download:notifications"
        );
    }
}
