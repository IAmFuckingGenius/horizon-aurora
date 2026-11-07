//! Horizon on Android: the entry points NativeActivity, its glue and the JVM look up by name.
//! What they start is horizon-client's (`horizon_client::android_host`): the launcher, the
//! platform glue and the game, one library with the native mod API.
#![cfg(target_os = "android")]

use horizon_client::android_host::{self, document_provider};
use jni::{
    JNIEnv,
    objects::{JClass, JString},
    sys::{jint, jstring},
};
use winit::platform::android::activity::{AndroidApp, OnCreateState};

/// NativeActivity's `onCreate`, before the event loop (the winit glue calls it).
#[unsafe(no_mangle)]
fn android_on_create(state: &OnCreateState<'_>) {
    android_host::on_create(state);
}

/// The event loop's thread (android-activity calls it).
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_host::main(app);
}

/// `HorizonDocumentsProvider.nativeCall`.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
extern "system" fn Java_dev_horizon_android_HorizonDocumentsProvider_nativeCall(
    env: JNIEnv<'_>,
    class: JClass<'_>,
    home: JString<'_>,
    operation: JString<'_>,
    id: JString<'_>,
    arg: JString<'_>,
) -> jstring {
    document_provider::Java_dev_horizon_android_HorizonDocumentsProvider_nativeCall(
        env, class, home, operation, id, arg,
    )
}

/// `HorizonDocumentsProvider.nativeOpen`.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
extern "system" fn Java_dev_horizon_android_HorizonDocumentsProvider_nativeOpen(
    env: JNIEnv<'_>,
    class: JClass<'_>,
    home: JString<'_>,
    id: JString<'_>,
    mode: JString<'_>,
) -> jint {
    document_provider::Java_dev_horizon_android_HorizonDocumentsProvider_nativeOpen(
        env, class, home, id, mode,
    )
}
