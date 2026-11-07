//! Installed engines: game libraries built on a computer (`tools/android_engine_pack.py`,
//! a `.hengine` ZIP of `engine.json` and `libhorizon_android.so`) that the boot library
//! (`crates/horizon-boot`) starts in place of the APK's own engine for one launch. Each
//! lives in `files/engines/<id>/`. Starting one writes the boot library's request and
//! restarts the app, since a process holds one engine.
use horizon_boot::{BUILT_IN, ENGINES, ERROR, LIBRARY, REQUEST, TAKEN, valid_id};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
};

/// The `engine.json` format this launcher reads.
pub const FORMAT: u32 = 1;
/// The largest engine library accepted, in bytes.
const LIMIT: u64 = 512 << 20;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub dirty: bool,
    pub abi: String,
    #[serde(default)]
    pub built: String,
    pub size: u64,
    pub sha256: String,
    /// The shell level of the tree it was built from (`horizon_boot::SHELL`); 0 for
    /// engines packed before levels existed.
    #[serde(default)]
    pub shell: u32,
}

/// An engine installed on this device.
#[derive(Clone, Debug)]
pub struct Engine {
    pub manifest: Manifest,
}

/// This build's ABI as Android names it.
pub fn abi() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64-v8a",
        "arm" => "armeabi-v7a",
        "x86_64" => "x86_64",
        "x86" => "x86",
        _ => "unknown",
    }
}
/// ELF `e_machine` for an ABI.
fn machine(abi: &str) -> Option<u16> {
    Some(match abi {
        "arm64-v8a" => 183,
        "armeabi-v7a" => 40,
        "x86_64" => 62,
        "x86" => 3,
        _ => return None,
    })
}

pub fn folder(home: &Path) -> PathBuf {
    home.join(ENGINES)
}

/// The installed engines, newest build first.
pub fn list(home: &Path) -> Vec<Engine> {
    let mut out: Vec<Engine> = fs::read_dir(folder(home))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let dir = e.path();
            let manifest: Manifest =
                serde_json::from_slice(&fs::read(dir.join("engine.json")).ok()?).ok()?;
            let name = dir.file_name()?.to_str()?;
            (manifest.id == name && dir.join(LIBRARY).is_file()).then_some(Engine { manifest })
        })
        .collect();
    out.sort_by(|a, b| b.manifest.built.cmp(&a.manifest.built));
    out
}

/// Why an engine file is refused, in English for `t_dyn`.
fn check(manifest: &Manifest) -> Result<(), String> {
    if manifest.format != FORMAT {
        return Err(
            "This engine was packed for another launcher version. Pack it again with this tree's tools/android_engine_pack.py."
                .into(),
        );
    }
    if !valid_id(&manifest.id) || manifest.id == BUILT_IN {
        return Err("The engine's id is not a valid folder name.".into());
    }
    if manifest.abi != abi() {
        return Err(format!(
            "This engine is built for {}, and this device needs {}.",
            manifest.abi,
            abi()
        ));
    }
    if manifest.size == 0 || manifest.size > LIMIT {
        return Err("The engine library has an impossible size.".into());
    }
    if !checksum(&manifest.sha256) {
        return Err("The engine has no valid checksum.".into());
    }
    if manifest.shell > shell() {
        return Err(NEEDS_NEWER_APP.into());
    }
    Ok(())
}
fn checksum(sha256: &str) -> bool {
    sha256.len() == 64 && sha256.chars().all(|c| c.is_ascii_hexdigit())
}
const NEEDS_NEWER_APP: &str =
    "This engine needs a newer version of the app. Update Horizon Aurora first.";

/// The app's shell level: what its boot library exports (`horizon_boot::SHELL` of the
/// APK, whichever engine runs), 0 for an app from before levels.
pub fn shell() -> u32 {
    apk_shell().unwrap_or(0)
}
#[cfg(target_os = "android")]
fn apk_shell() -> Option<u32> {
    // Bionic's flags: RTLD_NOW is 0 on 32-bit ABIs.
    const NOW: std::ffi::c_int = if cfg!(target_pointer_width = "64") {
        2
    } else {
        0
    };
    const NOLOAD: std::ffi::c_int = 4;
    #[link(name = "dl")]
    unsafe extern "C" {
        fn dlopen(name: *const std::ffi::c_char, flags: std::ffi::c_int) -> *mut std::ffi::c_void;
        fn dlsym(
            handle: *mut std::ffi::c_void,
            name: *const std::ffi::c_char,
        ) -> *mut std::ffi::c_void;
    }
    // NativeActivity loaded the boot library before any engine: find it, do not load it.
    let handle = unsafe { dlopen(c"libhorizon_boot.so".as_ptr(), NOW | NOLOAD) };
    if handle.is_null() {
        return None;
    }
    let level = unsafe { dlsym(handle, c"HORIZON_BOOT_SHELL".as_ptr()) };
    (!level.is_null()).then(|| unsafe { *level.cast::<u32>() })
}
#[cfg(not(target_os = "android"))]
fn apk_shell() -> Option<u32> {
    Some(horizon_boot::SHELL)
}

/// Where Aurora publishes engines: each release, and the rolling `dev` pre-release,
/// carries an `engines.json` beside its APKs (`tools/android_engine_index.py`).
pub const STORE: &str = "https://github.com/IAmFuckingGenius/horizon-aurora/releases/";
/// The lists: the latest release's, then the dev builds'.
pub const LISTS: [&str; 2] = [
    "https://github.com/IAmFuckingGenius/horizon-aurora/releases/latest/download/engines.json",
    "https://github.com/IAmFuckingGenius/horizon-aurora/releases/download/dev/engines.json",
];

/// An engine offered for download: an entry of an `engines.json`, with the `.hengine`
/// file's address, size and SHA-256.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct Offer {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub commit: String,
    pub abi: String,
    #[serde(default)]
    pub shell: u32,
    #[serde(default)]
    pub built: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

/// The offers in an `engines.json` this device can take: its ABI, a file in Aurora's
/// releases, a sane size and a checksum.
pub fn offers(list: &[u8]) -> Result<Vec<Offer>, String> {
    #[derive(serde::Deserialize)]
    struct List {
        format: u32,
        #[serde(default)]
        engines: Vec<Offer>,
    }
    let list: List = serde_json::from_slice(list)
        .map_err(|_| "The list of engines cannot be read.".to_owned())?;
    if list.format != FORMAT {
        return Err("The list of engines is for another version of the app.".into());
    }
    Ok(list
        .engines
        .into_iter()
        .filter(|o| {
            o.abi == abi()
                && valid_id(&o.id)
                && o.id != BUILT_IN
                && o.url
                    .strip_prefix(STORE)
                    .is_some_and(|rest| !rest.contains("..") && rest.starts_with("download/"))
                && (1..=LIMIT).contains(&o.size)
                && checksum(&o.sha256)
        })
        .collect())
}

/// What an offer is to this app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfferState {
    /// Not installed.
    New,
    /// Installed in an older build.
    Update,
    /// Installed in this build or a newer one.
    Installed,
    /// The engine the app carries itself.
    BuiltIn,
    /// Built for a higher shell level than this app's.
    NeedsNewerApp,
}
/// The state of `offer` beside the `installed` engines, for an app with the engine of
/// `built_in` (its commit) and shell level `shell`.
pub fn offer_state(offer: &Offer, installed: &[Engine], built_in: &str, shell: u32) -> OfferState {
    if offer.shell > shell {
        return OfferState::NeedsNewerApp;
    }
    if !offer.commit.is_empty() && offer.commit == built_in {
        return OfferState::BuiltIn;
    }
    match installed.iter().find(|e| e.manifest.id == offer.id) {
        None => OfferState::New,
        Some(e) if e.manifest.commit == offer.commit || e.manifest.built >= offer.built => {
            OfferState::Installed
        }
        Some(_) => OfferState::Update,
    }
}

/// The engine each published build of the app carries, named in its release's list
/// (`.github/workflows/android-release.yml`, `android-dev.yml`).
const RELEASE_ENGINE: &str = "aurora";
const DEV_ENGINE: &str = "aurora-dev";

/// Which builds of the app a build looks for newer ones of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// A release: the latest release.
    Release,
    /// A dev build (`-dev.<label>`): the rolling dev pre-release.
    Dev,
}
/// The channel of an app of `version` (`backend::VERSION`); none for a local build
/// (`-dev` with no label), which has nothing published to compare with.
pub fn channel(version: &str) -> Option<Channel> {
    match version.split_once("-dev") {
        None => Some(Channel::Release),
        Some((_, label)) if label.len() > 1 && label.starts_with('.') => Some(Channel::Dev),
        Some(_) => None,
    }
}
/// Where a newer build of the app is downloaded: the release's page.
pub fn release_page(channel: Channel) -> &'static str {
    match channel {
        Channel::Release => "https://github.com/IAmFuckingGenius/horizon-aurora/releases/latest",
        Channel::Dev => "https://github.com/IAmFuckingGenius/horizon-aurora/releases/tag/dev",
    }
}
/// A newer build of the app than one of `version` built from `commit`, among the
/// `offers` of the lists: every published build carries its engine, so the release
/// engine's version is that release's, and the dev engine's commit the dev build's.
/// A release looks for a later version; a dev build for a dev build of another commit.
pub fn app_update<'a>(offers: &'a [Offer], version: &str, commit: &str) -> Option<&'a Offer> {
    match channel(version)? {
        Channel::Release => offers
            .iter()
            .find(|o| o.id == RELEASE_ENGINE && newer_version(&o.version, version)),
        Channel::Dev => {
            let known = commit.len() >= 7 && commit.bytes().all(|b| b.is_ascii_hexdigit());
            offers.iter().find(|o| {
                o.id == DEV_ENGINE
                    && known
                    && o.commit.len() >= 7
                    && !o.commit.starts_with(commit)
                    && !commit.starts_with(&o.commit)
            })
        }
    }
}
/// Whether version `a` is later than `b`, number by number: "0.7.10-aurora.1" is later
/// than "0.7.9-aurora.2", and "0.7.7-aurora.2" than "0.7.7-aurora.1".
pub fn newer_version(a: &str, b: &str) -> bool {
    fn numbers(version: &str) -> Vec<u64> {
        version
            .split(|c: char| !c.is_ascii_digit())
            .filter(|n| !n.is_empty())
            .map(|n| n.parse().unwrap_or(u64::MAX))
            .collect()
    }
    numbers(a) > numbers(b)
}

/// Installs the `.hengine` in `file` (replacing an engine with the same id), checking
/// its manifest, ABI, ELF header, size and SHA-256. The library ends up read-only, as
/// Android 14 requires of code an app loads from its own files.
pub fn install(home: &Path, file: impl Read + Seek) -> Result<Manifest, String> {
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|_| "This file is not an engine: it is not a ZIP archive.".to_owned())?;
    let manifest: Manifest = {
        let entry = zip
            .by_name("engine.json")
            .map_err(|_| "This file is not an engine: it has no engine.json.".to_owned())?;
        let mut text = Vec::new();
        entry
            .take(64 << 10)
            .read_to_end(&mut text)
            .map_err(|e| e.to_string())?;
        serde_json::from_slice(&text)
            .map_err(|_| "The engine's engine.json cannot be read.".to_owned())?
    };
    check(&manifest)?;
    let engines = folder(home);
    fs::create_dir_all(&engines).map_err(|e| e.to_string())?;
    let partial = engines.join(format!(".{}.partial", manifest.id));
    let _ = fs::remove_dir_all(&partial);
    fs::create_dir_all(&partial).map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        let mut entry = zip
            .by_name(LIBRARY)
            .map_err(|_| format!("This engine has no {LIBRARY}."))?;
        let path = partial.join(LIBRARY);
        let mut out = fs::File::create(&path).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut head = Vec::with_capacity(20);
        let mut buffer = vec![0u8; 256 << 10];
        let mut total = 0u64;
        loop {
            let n = entry.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > manifest.size {
                return Err("The engine library is larger than its manifest says.".into());
            }
            if head.len() < 20 {
                head.extend_from_slice(&buffer[..n.min(20 - head.len())]);
            }
            hash.update(&buffer[..n]);
            out.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        }
        out.sync_all().map_err(|e| e.to_string())?;
        drop(out);
        if total != manifest.size {
            return Err("The engine library is incomplete.".into());
        }
        let digest: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if !digest.eq_ignore_ascii_case(&manifest.sha256) {
            return Err("The engine library is damaged: its checksum does not match.".into());
        }
        let elf_machine = (head.len() >= 20).then(|| u16::from_le_bytes([head[18], head[19]]));
        if !head.starts_with(b"\x7fELF") || elf_machine != machine(&manifest.abi) {
            return Err("The engine library is not a library for this device.".into());
        }
        set_read_only(&path)?;
        fs::write(
            partial.join("engine.json"),
            serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let dir = engines.join(&manifest.id);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        }
        fs::rename(&partial, &dir).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&partial);
    }
    result.map(|()| manifest)
}
fn set_read_only(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o444)).map_err(|e| e.to_string())
}

/// Removes an installed engine.
pub fn remove(home: &Path, id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("Invalid engine".into());
    }
    let dir = folder(home).join(id);
    // The library is read-only; its folder is not, so it can go.
    fs::remove_dir_all(&dir).map_err(|e| e.to_string())
}

/// Asks the boot library to start engine `id` (`None`: the APK's own) on the next
/// start, with `target` (what to open: the title screen, a world, a server, an
/// instance) for the engine to read.
pub fn request(home: &Path, id: Option<&str>, target: serde_json::Value) -> Result<(), String> {
    let id = match id {
        None => BUILT_IN,
        Some(id) if valid_id(id) && folder(home).join(id).join(LIBRARY).is_file() => id,
        Some(_) => return Err("This engine is not installed.".into()),
    };
    let engines = folder(home);
    fs::create_dir_all(&engines).map_err(|e| e.to_string())?;
    let json = serde_json::json!({ "engine": id, "target": target });
    let partial = engines.join(".launch.partial");
    fs::write(&partial, json.to_string()).map_err(|e| e.to_string())?;
    fs::rename(&partial, engines.join(REQUEST)).map_err(|e| e.to_string())
}

/// The request this start was made for (the boot library took it), read once.
pub fn take_started(home: &Path) -> Option<serde_json::Value> {
    let path = folder(home).join(TAKEN);
    let text = fs::read(&path).ok()?;
    let _ = fs::remove_file(&path);
    serde_json::from_slice(&text).ok()
}
/// The engine that failed to start and why, when the boot library fell back to the
/// APK's engine.
pub fn take_boot_error(home: &Path) -> Option<(String, String)> {
    let path = folder(home).join(ERROR);
    let text = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    let (id, reason) = text.split_once('\n').unwrap_or((&text, ""));
    Some((id.trim().to_owned(), short_paths(reason.trim())))
}
/// `reason` with each quoted absolute path (the linker quotes the library's) cut to
/// its file name, so the message fits a phone's dialog.
fn short_paths(reason: &str) -> String {
    let mut out = String::new();
    let mut rest = reason;
    while let Some(at) = rest.find("\"/") {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        let Some(end) = tail.find('"') else {
            out.push_str(&rest[at..]);
            return out;
        };
        let path = &tail[..end];
        out.push('«');
        out.push_str(path.rsplit('/').next().unwrap_or(path));
        out.push('»');
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The installed engine this code runs from, or `None` for the APK's engine.
pub fn running() -> Option<String> {
    let path = own_path()?;
    let dir = path.parent()?;
    (dir.parent()?.file_name()? == ENGINES)
        .then(|| dir.file_name()?.to_str().map(str::to_owned))
        .flatten()
}
#[cfg(target_os = "android")]
fn own_path() -> Option<PathBuf> {
    #[repr(C)]
    struct DlInfo {
        fname: *const std::ffi::c_char,
        fbase: *mut std::ffi::c_void,
        sname: *const std::ffi::c_char,
        saddr: *mut std::ffi::c_void,
    }
    #[link(name = "dl")]
    unsafe extern "C" {
        fn dladdr(addr: *const std::ffi::c_void, info: *mut DlInfo) -> std::ffi::c_int;
    }
    let mut info = DlInfo {
        fname: std::ptr::null(),
        fbase: std::ptr::null_mut(),
        sname: std::ptr::null(),
        saddr: std::ptr::null_mut(),
    };
    let ok = unsafe { dladdr(own_path as *const std::ffi::c_void, &mut info) };
    if ok == 0 || info.fname.is_null() {
        return None;
    }
    let name = unsafe { std::ffi::CStr::from_ptr(info.fname) };
    Some(PathBuf::from(name.to_string_lossy().as_ref()))
}
#[cfg(not(target_os = "android"))]
fn own_path() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn library(abi: &str) -> Vec<u8> {
        let mut bytes = b"\x7fELF".to_vec();
        bytes.resize(18, 0);
        bytes.extend_from_slice(&machine(abi).unwrap().to_le_bytes());
        bytes.resize(4096, 7);
        bytes
    }
    fn pack(manifest: &Manifest, lib: &[u8]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut out);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("engine.json", options).unwrap();
        zip.write_all(&serde_json::to_vec(manifest).unwrap())
            .unwrap();
        zip.start_file(LIBRARY, options).unwrap();
        zip.write_all(lib).unwrap();
        zip.finish().unwrap();
        out.into_inner()
    }
    fn manifest(lib: &[u8]) -> Manifest {
        Manifest {
            format: FORMAT,
            id: "pvp-1".into(),
            name: "PvP".into(),
            version: "0.7.7".into(),
            commit: "abc".into(),
            dirty: false,
            abi: abi().into(),
            built: "2026-11-03T10:00:00Z".into(),
            size: lib.len() as u64,
            sha256: Sha256::digest(lib)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            shell: horizon_boot::SHELL,
        }
    }
    fn home(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("horizon-engines-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_engine_installs_read_only_lists_and_is_requested_for_one_start() {
        if abi() == "unknown" {
            return;
        }
        let home = home("install");
        let lib = library(abi());
        let m = manifest(&lib);
        assert_eq!(install(&home, Cursor::new(pack(&m, &lib))).unwrap(), m);
        let engines = list(&home);
        assert_eq!(engines.len(), 1);
        assert_eq!(engines[0].manifest.name, "PvP");
        let path = folder(&home).join("pvp-1").join(LIBRARY);
        assert_eq!(fs::read(&path).unwrap(), lib);
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        // A new build of the same id replaces it.
        let mut newer = lib.clone();
        newer.push(1);
        let mut m2 = manifest(&newer);
        m2.name = "PvP 2".into();
        install(&home, Cursor::new(pack(&m2, &newer))).unwrap();
        assert_eq!(list(&home)[0].manifest.name, "PvP 2");
        // The boot library's side of a request.
        request(&home, Some("pvp-1"), serde_json::json!({"world": "W"})).unwrap();
        let (id, library) = horizon_boot::take_request(&home).unwrap();
        assert_eq!((id.as_str(), library), ("pvp-1", Some(path.clone())));
        assert_eq!(take_started(&home).unwrap()["target"]["world"], "W");
        assert_eq!(take_started(&home), None);
        assert!(request(&home, Some("missing"), serde_json::Value::Null).is_err());
        // The APK's engine needs nothing installed.
        request(&home, None, serde_json::json!({"instance": 2})).unwrap();
        assert_eq!(
            horizon_boot::take_request(&home),
            Some((BUILT_IN.into(), None))
        );
        assert_eq!(take_started(&home).unwrap()["target"]["instance"], 2);
        remove(&home, "pvp-1").unwrap();
        assert!(list(&home).is_empty());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn a_boot_failure_reads_once_with_short_paths() {
        let home = home("boot-error");
        fs::create_dir_all(folder(&home)).unwrap();
        fs::write(
            folder(&home).join(ERROR),
            "bad\ndlopen failed: \"/data/user/0/app/files/engines/bad/libhorizon_android.so\" is too small\n",
        )
        .unwrap();
        assert_eq!(
            take_boot_error(&home),
            Some((
                "bad".into(),
                "dlopen failed: «libhorizon_android.so» is too small".into()
            ))
        );
        assert_eq!(take_boot_error(&home), None);
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn damaged_foreign_and_lying_engines_are_refused_and_leave_nothing() {
        if abi() == "unknown" {
            return;
        }
        let home = home("refuse");
        let lib = library(abi());
        let mut bad_sum = manifest(&lib);
        bad_sum.sha256 = "0".repeat(64);
        let mut other_abi = manifest(&lib);
        other_abi.abi = if abi() == "x86_64" {
            "arm64-v8a"
        } else {
            "x86_64"
        }
        .into();
        let mut escape = manifest(&lib);
        escape.id = "../x".into();
        let mut short = manifest(&lib);
        short.size += 10;
        let mut future = manifest(&lib);
        future.format = FORMAT + 1;
        let mut newer_app = manifest(&lib);
        newer_app.shell = horizon_boot::SHELL + 1;
        let not_elf = vec![0u8; 64];
        for (name, file) in [
            ("checksum", pack(&bad_sum, &lib)),
            ("abi", pack(&other_abi, &lib)),
            ("id", pack(&escape, &lib)),
            ("size", pack(&short, &lib)),
            ("format", pack(&future, &lib)),
            ("shell", pack(&newer_app, &lib)),
            ("elf", pack(&manifest(&not_elf), &not_elf)),
            ("zip", b"hello".to_vec()),
        ] {
            assert!(install(&home, Cursor::new(file)).is_err(), "{name}");
        }
        assert!(list(&home).is_empty());
        let leftovers: Vec<_> = fs::read_dir(folder(&home))
            .map(|d| d.flatten().map(|e| e.file_name()).collect())
            .unwrap_or_default();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        fs::remove_dir_all(home).unwrap();
    }

    fn offer(id: &str, commit: &str, built: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id, "name": id, "version": "0.7.7-aurora.1", "commit": commit,
            "abi": abi(), "shell": horizon_boot::SHELL, "built": built,
            "url": format!("{STORE}download/dev/{id}-{}.hengine", abi()),
            "size": 1000, "sha256": "ab".repeat(32),
        })
    }

    #[test]
    fn a_list_offers_only_this_devices_engines_from_auroras_releases() {
        let good = offer("aurora-dev", "c1", "2026-11-06T10:00:00Z");
        let mut other_abi = offer("x86-only", "c1", "");
        other_abi["abi"] = (if abi() == "x86" { "arm64-v8a" } else { "x86" }).into();
        let mut elsewhere = offer("elsewhere", "c1", "");
        elsewhere["url"] = "https://example.com/releases/download/dev/e.hengine".into();
        let mut climbing = offer("climbing", "c1", "");
        climbing["url"] = format!("{STORE}download/../../evil.hengine").into();
        let mut no_sum = offer("no-sum", "c1", "");
        no_sum["sha256"] = "zz".into();
        let mut huge = offer("huge", "c1", "");
        huge["size"] = (LIMIT + 1).into();
        let built_in_id = offer(BUILT_IN, "c1", "");
        let list = serde_json::json!({"format": FORMAT, "engines": [
            good, other_abi, elsewhere, climbing, no_sum, huge, built_in_id,
        ]});
        let found = offers(list.to_string().as_bytes()).unwrap();
        if abi() == "unknown" {
            assert!(found.is_empty());
            return;
        }
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].id, "aurora-dev");
        assert!(offers(br#"{"format": 99, "engines": []}"#).is_err());
        assert!(offers(b"<html>").is_err());
    }

    #[test]
    fn an_offer_is_new_an_update_installed_built_in_or_too_new() {
        let o: Offer =
            serde_json::from_value(offer("aurora-dev", "c2", "2026-11-06T10:00:00Z")).unwrap();
        let shell = horizon_boot::SHELL;
        let installed = |commit: &str, built: &str| Engine {
            manifest: Manifest {
                id: "aurora-dev".into(),
                commit: commit.into(),
                built: built.into(),
                ..manifest(b"x")
            },
        };
        assert_eq!(offer_state(&o, &[], "c0", shell), OfferState::New);
        let older = installed("c1", "2026-11-05T10:00:00Z");
        assert_eq!(offer_state(&o, &[older], "c0", shell), OfferState::Update);
        let same = installed("c2", "2026-11-06T10:00:00Z");
        assert_eq!(offer_state(&o, &[same], "c0", shell), OfferState::Installed);
        let newer = installed("c3", "2026-11-07T10:00:00Z");
        assert_eq!(
            offer_state(&o, &[newer], "c0", shell),
            OfferState::Installed
        );
        // The release engine the app already carries is not offered again.
        assert_eq!(offer_state(&o, &[], "c2", shell), OfferState::BuiltIn);
        assert_eq!(
            offer_state(&o, &[], "c0", shell - 1),
            OfferState::NeedsNewerApp
        );
    }

    fn listed(id: &str, version: &str, commit: &str) -> Offer {
        let mut o = offer(id, commit, "2026-11-08T10:00:00Z");
        o["version"] = version.into();
        serde_json::from_value(o).unwrap()
    }

    #[test]
    fn versions_compare_number_by_number() {
        assert!(newer_version("0.7.8-aurora.1", "0.7.7-aurora.1"));
        assert!(newer_version("0.7.7-aurora.2", "0.7.7-aurora.1"));
        assert!(newer_version("0.7.10-aurora.1", "0.7.9-aurora.2"));
        assert!(newer_version("0.8.0", "0.7.7-aurora.1"));
        assert!(!newer_version("0.7.7-aurora.1", "0.7.7-aurora.1"));
        assert!(!newer_version("0.7.6-aurora.3", "0.7.7-aurora.1"));
    }

    #[test]
    fn builds_follow_their_own_channel() {
        assert_eq!(channel("0.7.7-aurora.1"), Some(Channel::Release));
        assert_eq!(channel("0.7.7-aurora.1-dev.12"), Some(Channel::Dev));
        // A local build has nothing published to compare with.
        assert_eq!(channel("0.7.7-aurora.1-dev"), None);
    }

    #[test]
    fn a_release_is_told_of_a_later_release_only() {
        let later = [listed("aurora", "0.7.8-aurora.1", "aaaaaaaaaaaa")];
        let same = [listed("aurora", "0.7.7-aurora.1", "aaaaaaaaaaaa")];
        let dev = [listed(
            "aurora-dev",
            "0.7.9-aurora.1-dev.4.bbbbbbbbbbbb",
            "bbbbbbbbbbbb",
        )];
        let found = app_update(&later, "0.7.7-aurora.1", "cccccccccccc");
        assert_eq!(found.map(|o| o.version.as_str()), Some("0.7.8-aurora.1"));
        assert!(app_update(&same, "0.7.7-aurora.1", "cccccccccccc").is_none());
        // Dev builds are not offered to a release.
        assert!(app_update(&dev, "0.7.7-aurora.1", "cccccccccccc").is_none());
    }

    #[test]
    fn a_dev_build_is_told_of_a_dev_build_of_another_commit() {
        let list = [
            listed("aurora", "0.7.8-aurora.1", "aaaaaaaaaaaa"),
            listed(
                "aurora-dev",
                "0.7.7-aurora.1-dev.5.bbbbbbbbbbbb",
                "bbbbbbbbbbbb",
            ),
        ];
        let found = app_update(&list, "0.7.7-aurora.1-dev.4", "cccccccccccc");
        assert_eq!(found.map(|o| o.id.as_str()), Some("aurora-dev"));
        // The same commit: up to date.
        assert!(app_update(&list, "0.7.7-aurora.1-dev.5", "bbbbbbbbbbbb").is_none());
        // A build from an unknown commit (a source export) cannot tell.
        assert!(app_update(&list, "0.7.7-aurora.1-dev.5", "source-export").is_none());
        // A local build is told of nothing.
        assert!(app_update(&list, "0.7.7-aurora.1-dev", "cccccccccccc").is_none());
    }
}
