use std::sync::OnceLock;

use jni::objects::{GlobalRef, JClass, JObject, JString};
use jni::{JNIEnv, JavaVM};
use serde_json::Value;

/// The plugin's own line to Kotlin (`NativeBridge.call`): the VM and the bridge's class, which
/// Kotlin hands over when the plugin loads. A class found from one of Rust's threads would come
/// from the system's class loader, which doesn't know the app's classes.
struct Bridge {
    vm: JavaVM,
    class: GlobalRef,
}

static BRIDGE: OnceLock<Bridge> = OnceLock::new();

/// Called by `app.raygun.audio.NativeBridge.onAttach` on the Android main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onAttach(env: JNIEnv, class: JClass) {
    if BRIDGE.get().is_some() {
        return;
    }
    if let (Ok(vm), Ok(class)) = (env.get_java_vm(), env.new_global_ref(&class)) {
        let _ = BRIDGE.set(Bridge { vm, class });
    }
}

/// Whether Kotlin handed over the bridge (it does as the plugin loads).
pub(crate) fn attached() -> bool {
    BRIDGE.get().is_some()
}

/// Runs one of the plugin's commands on its own line, from any of Rust's threads, with or
/// without an activity. Blocks until it answers.
pub(crate) fn call(command: &str, payload: &Value) -> Result<Value, String> {
    let bridge = BRIDGE.get().ok_or("Raygun's player isn't ready")?;
    let mut env = bridge
        .vm
        .attach_current_thread_permanently()
        .map_err(|error| error.to_string())?;
    let reply = answer(&mut env, &bridge.class, command, payload);
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    crate::parse_reply(&reply?)
}

fn answer(
    env: &mut JNIEnv,
    class: &GlobalRef,
    command: &str,
    payload: &Value,
) -> Result<String, String> {
    let to_text = |error: jni::errors::Error| error.to_string();
    let command = env.new_string(command).map_err(to_text)?;
    let args = env.new_string(payload.to_string()).map_err(to_text)?;
    let class: &JClass = class.as_obj().into();
    let reply = env
        .call_static_method(
            class,
            "call",
            "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
            &[(&command).into(), (&args).into()],
        )
        .and_then(|value| value.l())
        .map_err(to_text)?;
    let reply = JString::from(reply);
    let text: String = env.get_string(&reply).map_err(to_text)?.into();
    // Rust's threads stay attached: their local references are dropped here, not at detach.
    let _ = env.delete_local_ref(JObject::from(reply));
    let _ = env.delete_local_ref(JObject::from(command));
    let _ = env.delete_local_ref(JObject::from(args));
    Ok(text)
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

/// Called by `app.raygun.audio.NativeBridge.onWindowWanted` on the Android main thread: a new
/// activity came after the last one went, while Raygun carried on.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onWindowWanted(
    _env: JNIEnv,
    _class: JClass,
) {
    crate::deliver_window_wanted();
}

/// Called by `app.raygun.audio.NativeBridge.onWindowClosing` on the Android main thread, which
/// waits: the activity is about to be destroyed for good.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_raygun_audio_NativeBridge_onWindowClosing(
    _env: JNIEnv,
    _class: JClass,
) {
    crate::deliver_window_closing();
}
