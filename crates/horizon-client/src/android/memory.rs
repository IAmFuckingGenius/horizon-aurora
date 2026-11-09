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

/// Total memory, at most, of a phone that starts a new game folder with lighter settings
/// (`settings::set_low_memory_device`): 3 GiB phones report about 2.8 GiB, 4 GiB ones 3.6.
pub const LOW_MEMORY_TOTAL: u64 = 3 << 30;

/// The device's memory as the system counts it (`ActivityManager.MemoryInfo.totalMem`), or
/// `None` when JNI failed.
pub fn total_bytes() -> Option<u64> {
    crate::platform::with_context(|env, activity| {
        let service = jni::objects::JObject::from(env.new_string("activity")?);
        let manager = env
            .call_method(
                activity,
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::Object(&service)],
            )?
            .l()?;
        let info = env.new_object("android/app/ActivityManager$MemoryInfo", "()V", &[])?;
        env.call_method(
            &manager,
            "getMemoryInfo",
            "(Landroid/app/ActivityManager$MemoryInfo;)V",
            &[JValue::Object(&info)],
        )?;
        env.get_field(&info, "totalMem", "J")?.j()
    })
    .and_then(|total| u64::try_from(total).ok())
}
