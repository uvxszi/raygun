//! Media3's Transformer on Android (the `raygun-audio` plugin's `VideoConvert.kt`): a downloaded
//! video (a file, or a saved HLS playlist with its keys and segments) re-encoded to H.264 and AAC
//! in MP4 (or AAC alone in M4A) with the phone's encoders, what already fits copied. Rust starts
//! it, follows its progress and stops it; the plugin's `mediaCheck` reads the result back with
//! Android's own extractor and decoders.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use tauri::AppHandle;

/// What Android's player stack found in a converted file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Checked {
    pub seconds: f64,
    pub video: bool,
    pub audio: bool,
    /// A picture frame decoded.
    pub frame: bool,
}

#[cfg(target_os = "android")]
mod native {
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use serde_json::{Value, json};
    use tauri::{AppHandle, Manager, Wry};
    use tauri_plugin_raygun_audio::NativeAudio;

    use super::{Checked, Transform};

    /// Runs a blocking plugin call off the async threads.
    async fn plugin<T: Send + 'static>(
        app: &AppHandle,
        call: impl FnOnce(&NativeAudio<Wry>) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let audio = app
                .try_state::<NativeAudio<Wry>>()
                .ok_or("Raygun's native player isn't ready")?;
            call(audio.inner())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    pub async fn transform(
        app: &AppHandle,
        job: &Transform<'_>,
        cancel: &AtomicBool,
        progress: &(dyn Fn(f64) + Send + Sync),
    ) -> Result<(), String> {
        let session = uuid::Uuid::now_v7().to_string();
        let request = json!({
            "session": session,
            "input": job.input.to_string_lossy(),
            "output": job.output.to_string_lossy(),
            "audioOnly": job.audio_only,
            "stereo": job.stereo,
            "video": job.video,
            "audio": job.audio,
        });
        plugin(app, move |audio| audio.convert_start(&request)).await?;
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if cancel.load(Ordering::SeqCst) {
                let session = session.clone();
                let _ = plugin(app, move |audio| audio.convert_stop(&session)).await;
                return Err("Cancelled".into());
            }
            let asked = session.clone();
            let status: Value = plugin(app, move |audio| audio.convert_status(&asked)).await?;
            match status["state"].as_str() {
                Some("done") => return Ok(()),
                Some("failed") | None => {
                    return Err(status["error"]
                        .as_str()
                        .filter(|error| !error.is_empty())
                        .unwrap_or("The phone couldn't convert this video")
                        .to_owned());
                }
                _ => {
                    if let Some(share) = status["progress"].as_f64() {
                        progress((share / 100.0).clamp(0.0, 1.0));
                    }
                }
            }
        }
    }

    pub async fn check(app: &AppHandle, path: &Path) -> Result<Checked, String> {
        let path = path.to_string_lossy().into_owned();
        let found: Value = plugin(app, move |audio| audio.media_check(&path)).await?;
        Ok(Checked {
            seconds: found["durationMs"].as_f64().unwrap_or(0.0) / 1000.0,
            video: found["video"].as_bool().unwrap_or(false),
            audio: found["audio"].as_bool().unwrap_or(false),
            frame: found["frame"].as_bool().unwrap_or(false),
        })
    }
}

/// One run of Transformer.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub struct Transform<'a> {
    pub input: &'a Path,
    /// MP4, or M4A when `audio_only`.
    pub output: &'a Path,
    pub audio_only: bool,
    /// The sound mixed to stereo.
    pub stereo: bool,
    /// What the source has (both when unknown).
    pub video: bool,
    pub audio: bool,
}

/// Re-encodes `job.input` into `job.output`; `progress` hears the share done.
#[cfg(target_os = "android")]
pub async fn transform(
    app: &AppHandle,
    job: &Transform<'_>,
    cancel: &AtomicBool,
    progress: &(dyn Fn(f64) + Send + Sync),
) -> Result<(), String> {
    native::transform(app, job, cancel, progress).await
}

#[cfg(not(target_os = "android"))]
pub async fn transform(
    _app: &AppHandle,
    _job: &Transform<'_>,
    _cancel: &AtomicBool,
    _progress: &(dyn Fn(f64) + Send + Sync),
) -> Result<(), String> {
    Err("Re-encoding with the phone's encoders is only on Android".into())
}

/// What Android's extractor and decoders find in a converted file.
#[cfg(target_os = "android")]
pub async fn check(app: &AppHandle, path: &Path) -> Result<Checked, String> {
    native::check(app, path).await
}

#[cfg(not(target_os = "android"))]
pub async fn check(_app: &AppHandle, _path: &Path) -> Result<Checked, String> {
    Err("Android's player stack is only on Android".into())
}
