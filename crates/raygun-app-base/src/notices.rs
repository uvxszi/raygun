//! Notifications besides the downloads' own (`Notice`): news, releases, what waits for Raygun to
//! be open. raygun-app's `notifications` shows the downloads and takes the taps.

use tauri::AppHandle;

/// Where a [`Notice`] goes on Android: its channel, which people can mute on its own.
pub struct NoticeChannel {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Whether its notices may make a sound, vibrate and pop up. Only news does (new releases,
    /// chapters and episodes, Raygun's own update); everything else is silent. Android keeps a
    /// channel's importance as first made, so a channel that goes quiet takes a new id and its old
    /// one is listed in [`RETIRED_CHANNELS`].
    pub alerts: bool,
}

/// Followed artists' new releases (M46.42).
pub const RELEASES: NoticeChannel = NoticeChannel {
    id: "raygun.releases",
    name: "New releases",
    description: "Releases from artists you follow",
    alerts: true,
};

/// New chapters and episodes found by library updates and feed refreshes, one per title.
pub const LIBRARY_UPDATES: NoticeChannel = NoticeChannel {
    id: "raygun.library.updates",
    name: "Library updates",
    description: "New chapters and episodes in your library",
    alerts: true,
};

/// A newer Raygun, found by a background check.
pub const APP_UPDATES: NoticeChannel = NoticeChannel {
    id: "raygun.app.updates",
    name: "Raygun updates",
    description: "A new version of Raygun",
    alerts: true,
};

/// What waits for Raygun to be open: a song in YouTube's player paused, songs to record.
pub const PLAYBACK: NoticeChannel = NoticeChannel {
    id: "raygun.playback.quiet",
    name: "Playback",
    description: "Songs that wait for Raygun to be open",
    alerts: false,
};

/// Files going to or coming from a linked device (M46.57).
pub const DEVICES: NoticeChannel = NoticeChannel {
    id: "raygun.devices",
    name: "Linked devices",
    description: "Files sent to and from your devices",
    alerts: false,
};

/// Every channel [`Notice`]s use.
#[cfg_attr(not(test), allow(dead_code))]
pub const CHANNELS: [&NoticeChannel; 5] = [
    &RELEASES,
    &LIBRARY_UPDATES,
    &APP_UPDATES,
    &PLAYBACK,
    &DEVICES,
];

/// Channel ids no longer used (made with a sound before they went quiet): the plugin deletes them.
pub const RETIRED_CHANNELS: [&str; 1] = ["raygun.playback"];

/// A notification besides the downloads' own that opens a screen when tapped.
pub struct Notice {
    /// Tells it from the others of its channel: the same id replaces the one shown.
    pub id: i32,
    pub channel: &'static NoticeChannel,
    pub title: String,
    pub text: String,
    /// What a tap opens (see raygun-app's `DownloadNotices::open`).
    pub route: String,
}

impl Notice {
    /// This notice with both lines saying something (Android shows a notification without a
    /// title or text as an empty one): a blank title is its channel's name, a blank text its
    /// channel's description. `None` when it says nothing at all: it isn't shown.
    pub fn filled(self) -> Option<Notice> {
        let title = self.title.trim();
        let text = self.text.trim();
        if title.is_empty() && text.is_empty() {
            return None;
        }
        Some(Notice {
            title: if title.is_empty() {
                self.channel.name.to_owned()
            } else {
                title.to_owned()
            },
            text: if text.is_empty() {
                self.channel.description.to_owned()
            } else {
                text.to_owned()
            },
            ..self
        })
    }

    /// What the `raygun-audio` plugin shows on Android.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "channel": self.channel.id,
            "channelName": self.channel.name,
            "channelDescription": self.channel.description,
            "alerts": self.channel.alerts,
            "retiredChannels": RETIRED_CHANNELS,
            "title": self.title,
            "text": self.text,
            "route": self.route,
        })
    }
}

/// A notification id for `key` that stays the same across runs (FNV-1a), never negative.
pub fn notice_id(key: &str) -> i32 {
    let hash = key.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    (hash & 0x7fff_ffff) as i32
}

/// Shows `notice`: on Android through the `raygun-audio` plugin, whose tap brings the route back
/// through raygun-app's `DownloadNotices::open`; elsewhere as a system notification carrying the route,
/// which a click opens while it's on screen.
pub async fn show_notice(app: &AppHandle, notice: Notice) {
    let id = notice.id;
    let Some(notice) = notice.filled() else {
        tracing::debug!(id, "a notification with nothing to say isn't shown");
        return;
    };
    if let Err(error) = platform::notice(app, &notice).await {
        tracing::debug!(%error, title = %notice.title, "couldn't show a notification");
    }
}

/// Takes away the notification [`show_notice`] showed with `id`: what it said is over (Android;
/// desktop notifications go on their own).
pub async fn cancel_notice(app: &AppHandle, id: i32) {
    if let Err(error) = platform::cancel_notice(app, id).await {
        tracing::debug!(%error, id, "couldn't take a notification away");
    }
}

#[cfg(target_os = "android")]
mod platform {
    use tauri::{AppHandle, Manager};

    pub async fn notice(app: &AppHandle, notice: &super::Notice) -> Result<(), String> {
        let value = notice.to_json();
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .notice(&value)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    pub async fn cancel_notice(app: &AppHandle, id: i32) -> Result<(), String> {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .notice_cancel(id)
        })
        .await
        .map_err(|error| error.to_string())?
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    use tauri::AppHandle;
    use tauri_plugin_notification::NotificationExt;

    /// Without a sound named, Windows shows a toast silently: only channels that alert get one.
    pub async fn notice(app: &AppHandle, notice: &super::Notice) -> Result<(), String> {
        let mut builder = app
            .notification()
            .builder()
            .id(notice.id)
            .title(&notice.title)
            .body(&notice.text)
            .extra("route", &notice.route);
        if notice.channel.alerts {
            builder = builder.sound("Default");
        }
        builder.show().map_err(|error| error.to_string())
    }

    /// The system keeps desktop notifications in its own list, which the person clears.
    pub async fn cancel_notice(_app: &AppHandle, _id: i32) -> Result<(), String> {
        Ok(())
    }
}
