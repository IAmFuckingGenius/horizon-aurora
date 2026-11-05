//! Horizon Android's boot library. NativeActivity loads this small library first
//! (`android.app.lib_name` is `horizon_boot`), and it hands the activity to an engine:
//! the full game library, `libhorizon_android.so`. That is the APK's own engine, or,
//! for one launch, an engine the player installed in `files/engines/<id>/`, which the
//! launcher asks for by writing `files/engines/launch.json` and restarting the app.
//!
//! The request is taken (renamed to `launch.taken.json`, where the engine reads what
//! to start) before the engine loads, so an engine that crashes leaves the next start
//! on the APK's engine. An installed engine that cannot load leaves its reason in
//! `files/engines/boot-error.txt`, and the APK's engine starts instead.
use std::path::{Path, PathBuf};

/// The engines' folder in the app's files.
pub const ENGINES: &str = "engines";
/// A launch request: `{"engine": "<id>", ...}`.
pub const REQUEST: &str = "launch.json";
/// The request once the boot library has taken it.
pub const TAKEN: &str = "launch.taken.json";
/// Why the requested engine did not load, for the launcher to show.
pub const ERROR: &str = "boot-error.txt";
/// The engine library's file name, in the APK and in each engine's folder.
pub const LIBRARY: &str = "libhorizon_android.so";
/// The engine id of a request for the APK's own engine (to open an instance on it
/// from an installed engine). No installed engine may take this id.
pub const BUILT_IN: &str = "builtin";
/// The APK's shell level: what it gives an engine besides this library (its Java side,
/// the documents provider, the manifest). An engine records the level of the tree it
/// was built from (`shell` in `engine.json`), and the launcher installs only engines up
/// to the app's own level. Bump it when the Java side changes; CI fails until then
/// (`packaging/android/shell.json`, checked by `tools/test_android_engines.py`).
pub const SHELL: u32 = 1;

/// Takes the launch request in `files` (the app's files directory): the requested
/// engine's id and its library, `None` for the APK's own engine, when the request
/// names a valid engine.
pub fn take_request(files: &Path) -> Option<(String, Option<PathBuf>)> {
    let engines = files.join(ENGINES);
    let request = engines.join(REQUEST);
    let text = std::fs::read_to_string(&request).ok()?;
    // Taken first: whatever happens next, the next start does not repeat it.
    if std::fs::rename(&request, engines.join(TAKEN)).is_err() {
        let _ = std::fs::remove_file(&request);
    }
    let id = string_field(&text, "engine").filter(|id| valid_id(id))?;
    let library = (id != BUILT_IN).then(|| engines.join(&id).join(LIBRARY));
    Some((id, library))
}

/// An engine id is a folder name: letters, digits, `-`, `_` and `.`, not starting
/// with a dot, at most 64 characters.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// The string value of `"key"` in a flat JSON object, without escapes (ids have none).
fn string_field(json: &str, key: &str) -> Option<String> {
    let quoted = format!("\"{key}\"");
    let at = json.find(&quoted)? + quoted.len();
    let rest = json[at..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let value = &rest[..end];
    (!value.contains('\\')).then(|| value.to_owned())
}

#[cfg(all(target_os = "android", feature = "entry"))]
mod android {
    use std::ffi::{CStr, CString, c_char, c_int, c_void};
    use std::path::{Path, PathBuf};

    /// `ANativeActivity` from the NDK's `native_activity.h`, up to the fields used.
    #[repr(C)]
    pub struct ANativeActivity {
        callbacks: *mut c_void,
        vm: *mut c_void,
        env: *mut c_void,
        clazz: *mut c_void,
        internal_data_path: *const c_char,
    }
    type OnCreate = unsafe extern "C" fn(*mut ANativeActivity, *mut c_void, usize);

    /// The APK's shell level, read by the engine it starts (`engines::shell`).
    #[unsafe(no_mangle)]
    pub static HORIZON_BOOT_SHELL: u32 = super::SHELL;

    const RTLD_NOW: c_int = 2;
    const INFO: c_int = 4;
    const ERROR: c_int = 6;
    #[link(name = "dl")]
    unsafe extern "C" {
        fn dlopen(name: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        fn dlerror() -> *const c_char;
    }
    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(priority: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }
    fn log(priority: c_int, text: &str) {
        if let Ok(text) = CString::new(text) {
            unsafe { __android_log_write(priority, c"HorizonBoot".as_ptr(), text.as_ptr()) };
        }
    }
    /// The engine at `name` (a path, or the APK's library by its file name) and its
    /// entry point, or why it cannot start.
    fn open(name: &str) -> Result<OnCreate, String> {
        let name = CString::new(name).map_err(|e| e.to_string())?;
        let reason = || {
            let e = unsafe { dlerror() };
            if e.is_null() {
                "unknown error".to_owned()
            } else {
                unsafe { CStr::from_ptr(e) }.to_string_lossy().into_owned()
            }
        };
        let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
        if handle.is_null() {
            return Err(reason());
        }
        let entry = unsafe { dlsym(handle, c"ANativeActivity_onCreate".as_ptr()) };
        if entry.is_null() {
            return Err(reason());
        }
        Ok(unsafe { std::mem::transmute::<*mut c_void, OnCreate>(entry) })
    }
    fn files(activity: *mut ANativeActivity) -> Option<PathBuf> {
        let path = unsafe { (*activity).internal_data_path };
        (!path.is_null())
            .then(|| PathBuf::from(unsafe { CStr::from_ptr(path) }.to_string_lossy().as_ref()))
    }

    /// NativeActivity's entry point: picks the engine and calls its own.
    ///
    /// # Safety
    /// Called by NativeActivity with its activity and saved state.
    #[allow(non_snake_case)]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn ANativeActivity_onCreate(
        activity: *mut ANativeActivity,
        saved: *mut c_void,
        saved_size: usize,
    ) {
        let mut entry = None;
        if let Some(files) = files(activity) {
            let engines = files.join(super::ENGINES);
            let _ = std::fs::remove_file(engines.join(super::ERROR));
            if let Some((id, Some(library))) = super::take_request(&files) {
                match open(&library.to_string_lossy()) {
                    Ok(on_create) => {
                        log(INFO, &format!("Starting installed engine {id}"));
                        entry = Some(on_create);
                    }
                    Err(reason) => {
                        log(ERROR, &format!("Engine {id} cannot start: {reason}"));
                        write_error(&engines, &id, &reason);
                    }
                }
            }
        }
        let entry = match entry {
            Some(entry) => entry,
            None => match open(super::LIBRARY) {
                Ok(on_create) => on_create,
                Err(reason) => {
                    log(ERROR, &format!("The APK's engine cannot start: {reason}"));
                    return;
                }
            },
        };
        unsafe { entry(activity, saved, saved_size) };
    }
    fn write_error(engines: &Path, id: &str, reason: &str) {
        let _ = std::fs::write(engines.join(super::ERROR), format!("{id}\n{reason}\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("horizon-boot-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(ENGINES)).unwrap();
        dir
    }

    #[test]
    fn a_request_is_taken_once_and_names_the_engines_library() {
        let dir = files("take");
        let request = dir.join(ENGINES).join(REQUEST);
        std::fs::write(&request, r#"{ "engine" : "pvp-1.2", "instance": "a" }"#).unwrap();
        let (id, library) = take_request(&dir).unwrap();
        assert_eq!(id, "pvp-1.2");
        assert_eq!(
            library,
            Some(dir.join("engines/pvp-1.2/libhorizon_android.so"))
        );
        // The engine reads what to start from the taken request; a second start
        // finds none and boots the APK's engine.
        assert!(!request.exists());
        assert!(dir.join(ENGINES).join(TAKEN).exists());
        assert_eq!(take_request(&dir), None);
        // The APK's own engine: no library to load, and the request stays for it.
        std::fs::write(&request, r#"{"engine": "builtin", "target": {}}"#).unwrap();
        assert_eq!(take_request(&dir), Some(("builtin".into(), None)));
        assert!(dir.join(ENGINES).join(TAKEN).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_request_cannot_leave_the_engines_folder() {
        let dir = files("escape");
        for bad in [
            r#"{"engine": "../../lib"}"#,
            r#"{"engine": ".hidden"}"#,
            r#"{"engine": "a/b"}"#,
            r#"{"engine": ""}"#,
            r#"{"engine": "a\"b"}"#,
            r#"{"other": "x"}"#,
            "not json",
        ] {
            std::fs::write(dir.join(ENGINES).join(REQUEST), bad).unwrap();
            assert_eq!(take_request(&dir), None, "{bad}");
            assert!(
                !dir.join(ENGINES).join(REQUEST).exists(),
                "{bad} was not taken"
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
