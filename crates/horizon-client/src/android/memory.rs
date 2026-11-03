//! Android's memory pressure on this process: the last `onTrimMemory` level the system
//! sent it, read with `ActivityManager.getMyMemoryState` (NativeActivity forwards only
//! `onLowMemory`, which winit reports as a memory warning).
use jni::objects::JValue;

/// `ComponentCallbacks2` levels.
pub const RUNNING_LOW: i32 = 10;
pub const RUNNING_CRITICAL: i32 = 15;
pub const UI_HIDDEN: i32 = 20;
pub const BACKGROUND: i32 = 40;

/// The process's `lastTrimLevel` (0 when the system sent none), or `None` when JNI failed.
pub fn trim_level() -> Option<i32> {
    crate::platform::with_context(|env, _activity| {
        let info = env.new_object(
            "android/app/ActivityManager$RunningAppProcessInfo",
            "()V",
            &[],
        )?;
        env.call_static_method(
            "android/app/ActivityManager",
            "getMyMemoryState",
            "(Landroid/app/ActivityManager$RunningAppProcessInfo;)V",
            &[JValue::Object(&info)],
        )?;
        env.get_field(&info, "lastTrimLevel", "I")?.i()
    })
}
