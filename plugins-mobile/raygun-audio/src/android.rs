use jni::JNIEnv;
use jni::objects::{JClass, JString};
use raygun_model::EngineCommand;
use serde_json::Value;
use tauri::Runtime;
use tauri::plugin::PluginHandle;

pub(crate) fn send<R: Runtime>(
    handle: &PluginHandle<R>,
    command: &EngineCommand,
) -> Result<(), String> {
    // The Kotlin commands are named after the `type` tag; the rest of the object is the payload.
    let mut payload = serde_json::to_value(command).map_err(|error| error.to_string())?;
    let name = match payload
        .as_object_mut()
        .and_then(|fields| fields.remove("type"))
    {
        Some(Value::String(name)) => name,
        _ => return Err("engine command has no type".into()),
    };
    handle
        .run_mobile_plugin::<Value>(&name, payload)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Called by `app.raygun.audio.NativeBridge.onControl` on the Android main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onControl(
    mut env: JNIEnv,
    _class: JClass,
    control: JString,
) {
    if let Ok(control) = env.get_string(&control) {
        crate::deliver_control(control.into());
    }
}

/// Called by `app.raygun.audio.NativeBridge.onTts` on the Android main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onTts(
    mut env: JNIEnv,
    _class: JClass,
    json: JString,
) {
    if let Ok(json) = env.get_string(&json) {
        crate::deliver_tts(json.into());
    }
}

/// Called by `app.raygun.audio.NativeBridge.onVideo` on the Android main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onVideo(
    mut env: JNIEnv,
    _class: JClass,
    json: JString,
) {
    if let Ok(json) = env.get_string(&json) {
        crate::deliver_video(json.into());
    }
}

/// Called by `app.raygun.audio.NativeBridge.onWebsite` on the Android main thread (M46.3).
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onWebsite(
    mut env: JNIEnv,
    _class: JClass,
    json: JString,
) {
    if let Ok(json) = env.get_string(&json) {
        crate::deliver_website(json.into());
    }
}

/// Called by `app.raygun.audio.NativeBridge.onNotificationAction` on the Android main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onNotificationAction(
    mut env: JNIEnv,
    _class: JClass,
    json: JString,
) {
    if let Ok(json) = env.get_string(&json) {
        crate::deliver_notification_action(json.into());
    }
}

/// Called by `app.raygun.audio.NativeBridge.onOpen` on the Android main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onOpen(
    mut env: JNIEnv,
    _class: JClass,
    route: JString,
) {
    if let Ok(route) = env.get_string(&route) {
        crate::deliver_open(route.into());
    }
}

/// Called by `app.raygun.audio.NativeBridge.onOpenWith` on a thread of the plugin's own.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onOpenWith(
    mut env: JNIEnv,
    _class: JClass,
    json: JString,
) {
    if let Ok(json) = env.get_string(&json) {
        crate::deliver_open_with(json.into());
    }
}

/// Called by `app.raygun.audio.NativeBridge.onEngineEvent` on the Android main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onEngineEvent(
    mut env: JNIEnv,
    _class: JClass,
    json: JString,
) {
    if let Ok(json) = env.get_string(&json) {
        crate::deliver_report(json.into());
    }
}

/// Called by `app.raygun.audio.NativeBridge.onDeviceState` on the Android main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onDeviceState(
    mut env: JNIEnv,
    _class: JClass,
    json: JString,
) {
    if let Ok(json) = env.get_string(&json) {
        crate::deliver_device_state(json.into());
    }
}
