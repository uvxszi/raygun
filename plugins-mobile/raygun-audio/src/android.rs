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
