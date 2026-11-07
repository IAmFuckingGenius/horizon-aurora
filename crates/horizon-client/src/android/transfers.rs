//! World ZIPs and reports through Android's system file picker (the Storage
//! Access Framework): ACTION_CREATE_DOCUMENT saves a world ZIP or a report where
//! the player chooses, ACTION_OPEN_DOCUMENT brings a world ZIP in, and
//! ACTION_SEND shares a report. The ZIP rules are `world_archive.rs`; this is the
//! Android glue, one background worker and the progress the launcher draws.
use crate::launcher::text::{t, t_dyn, tf};
use crate::world_archive::{self as archive, Progress};
use jni::{
    JNIEnv,
    objects::{JObject, JString, JValue},
};
use std::{
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

#[path = "horizon_folders.rs"]
mod horizon_folders;
#[path = "horizon_import.rs"]
mod horizon_import;

/// Activity request codes; `HorizonActivity` forwards 4101 to 4103. The engine and
/// Horizon Android imports reuse IMPORT_WORLD (the pending transfer says which), so
/// the APK's Java side, and with it its shell level, stays as it is.
pub const IMPORT_WORLD: i32 = 4102;
pub const EXPORT: i32 = 4103;
/// What file managers call a ZIP. A world ZIP from a browser or a PC can carry
/// any of these; the content is checked either way.
const ZIP_TYPES: [&str; 4] = [
    "application/zip",
    "application/x-zip-compressed",
    "application/x-zip",
    "application/octet-stream",
];
/// Space left free on the device after an import, beyond the world itself.
const MARGIN: u64 = 32 << 20;
/// The hidden folder beside a profile's `saves` where an import unpacks before
/// it moves in whole. The Files app never lists it.
const WORK: &str = ".world-transfer";

/// A report of Settings › Diagnostics & about.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Report {
    Diagnostics,
    Performance,
    LaunchLog,
}
impl Report {
    fn mime(self) -> &'static str {
        match self {
            Self::Diagnostics => "application/zip",
            // An unknown type keeps the `.jsonl` a provider would otherwise replace.
            Self::Performance => "application/octet-stream",
            Self::LaunchLog => "text/plain",
        }
    }
}
/// Where a report goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Delivery {
    /// ACTION_CREATE_DOCUMENT: a folder or drive the player picks.
    Save,
    /// ACTION_SEND: another app.
    Share,
    /// The Reports folder in the Files app, as before.
    Files,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Job {
    ExportWorld,
    ImportWorld,
    SaveFile,
    ImportEngine,
    /// Worlds, packs, mods and settings from Horizon Android or Starling.
    ImportHorizon,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// Copying a ZIP that can only be read front to back into the work folder.
    Receiving,
    Checking,
    Writing,
}
/// The transfer in progress, for the launcher's dialog.
#[derive(Clone)]
pub struct Running {
    pub job: Job,
    pub name: String,
    pub stage: Stage,
    pub progress: Progress,
    pub cancelling: bool,
}
/// A finished transfer, in the launcher's language.
pub struct Notice {
    pub title: String,
    pub detail: String,
    /// Shown in a dialog until dismissed, not as a toast.
    pub error: bool,
    /// The Worlds list changed.
    pub worlds: bool,
}

enum Pending {
    Import {
        saves: PathBuf,
    },
    /// An engine (`.hengine`) for `files/engines` under `home`.
    Engine {
        home: PathBuf,
    },
    World {
        dir: PathBuf,
        name: String,
    },
    File {
        path: PathBuf,
    },
    /// Horizon Android's folder, into a new instance with the settings of `from`.
    Horizon {
        home: PathBuf,
        from: u32,
    },
}
struct State {
    pending: Option<Pending>,
    running: Option<Running>,
    notices: Vec<Notice>,
}
static STATE: Mutex<State> = Mutex::new(State {
    pending: None,
    running: None,
    notices: Vec::new(),
});
static CANCEL: AtomicBool = AtomicBool::new(false);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}
fn notify(title: &'static str, detail: String, error: bool, worlds: bool) {
    state().notices.push(Notice {
        title: t(title).into(),
        detail,
        error,
        worlds,
    });
}

pub fn running() -> Option<Running> {
    state().running.clone()
}
pub fn poll() -> Option<Notice> {
    let mut s = state();
    (!s.notices.is_empty()).then(|| s.notices.remove(0))
}
/// Stops the running transfer at its next 64 KiB; it cleans up after itself.
pub fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
    if let Some(running) = state().running.as_mut() {
        running.cancelling = true;
    }
}

fn begin(pending: Pending, picker: crate::native_ime::Picker) -> Result<(), String> {
    if crate::documents::busy() {
        return Err(t("Finish the add-on import first").into());
    }
    {
        let mut s = state();
        if s.pending.is_some() || s.running.is_some() {
            return Err(t("Wait for the current transfer to finish").into());
        }
        s.pending = Some(pending);
    }
    crate::platform::hide();
    if !crate::native_ime::pick(picker) {
        state().pending = None;
        return Err(t("The system file picker is not available").into());
    }
    Ok(())
}
/// Asks where to save `dir` as `<name>.zip`, then writes the ZIP there.
pub fn export_world(dir: PathBuf, name: &str) -> Result<(), String> {
    let title = archive::export_file_name(name);
    begin(
        Pending::World {
            dir,
            name: name.into(),
        },
        crate::native_ime::Picker {
            request: EXPORT,
            tree: false,
            create: true,
            mimes: vec!["application/zip"],
            title,
        },
    )
}
/// Asks for a world ZIP, then unpacks it into `saves` under a free name.
pub fn import_world(saves: PathBuf) -> Result<(), String> {
    begin(
        Pending::Import { saves },
        crate::native_ime::Picker {
            request: IMPORT_WORLD,
            tree: false,
            create: false,
            mimes: ZIP_TYPES.to_vec(),
            title: String::new(),
        },
    )
}
/// Asks for Horizon Android's folder (or Starling's: the same app), then copies its
/// worlds, packs, mods and settings into a new instance (`horizon_import.rs`).
pub fn import_horizon(home: PathBuf, from: u32) -> Result<(), String> {
    begin(
        Pending::Horizon { home, from },
        crate::native_ime::Picker {
            request: IMPORT_WORLD,
            create: false,
            tree: true,
            mimes: Vec::new(),
            title: String::new(),
        },
    )
}
/// Asks for an engine file (`.hengine`), then installs it (`engines.rs`).
pub fn import_engine(home: PathBuf) -> Result<(), String> {
    begin(
        Pending::Engine { home },
        crate::native_ime::Picker {
            request: IMPORT_WORLD,
            tree: false,
            create: false,
            mimes: ZIP_TYPES.to_vec(),
            title: String::new(),
        },
    )
}
/// Asks where to save a copy of the report file `path`.
pub fn save_report(path: PathBuf, report: Report) -> Result<(), String> {
    let title = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    begin(
        Pending::File { path },
        crate::native_ime::Picker {
            request: EXPORT,
            tree: false,
            create: true,
            mimes: vec![report.mime()],
            title,
        },
    )
}
/// Called when Android could not show the picker.
pub fn picker_failed(_request: i32) {
    if state().pending.take().is_some() {
        notify(
            "Could not open the file picker",
            t("Android has no app to choose files with. Use Open in Files instead.").into(),
            true,
            false,
        );
    }
}

/// The Android share sheet for a report in Reports, through our own documents
/// provider: the receiving app gets read access to this one file only.
pub fn share_report(path: &Path, report: Report) -> Result<(), String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid report name")?;
    let id = format!("{}/{name}", crate::document_store::folder_id(0, "reports"));
    let subject = match report {
        Report::Diagnostics => t("Horizon Android report for developers"),
        Report::Performance => t("Horizon Android performance data"),
        Report::LaunchLog => t("Horizon Android launch log"),
    };
    let title = t("Share");
    crate::platform::with_context(|env, activity| {
        let uri = crate::platform::document_uri(env, activity, &id)?;
        let action = JObject::from(env.new_string("android.intent.action.SEND")?);
        let intent = env.new_object(
            "android/content/Intent",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&action)],
        )?;
        let mime = JObject::from(env.new_string(report.mime())?);
        env.call_method(
            &intent,
            "setType",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&mime)],
        )?;
        let stream = JObject::from(env.new_string("android.intent.extra.STREAM")?);
        env.call_method(
            &intent,
            "putExtra",
            "(Ljava/lang/String;Landroid/os/Parcelable;)Landroid/content/Intent;",
            &[JValue::Object(&stream), JValue::Object(&uri)],
        )?;
        let key = JObject::from(env.new_string("android.intent.extra.SUBJECT")?);
        let text = JObject::from(env.new_string(subject)?);
        env.call_method(
            &intent,
            "putExtra",
            "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&key), JValue::Object(&text)],
        )?;
        // The grant travels with ClipData; EXTRA_STREAM alone is not enough
        // through the chooser on every Android version.
        let label = JObject::from(env.new_string(name)?);
        let clip = env
            .call_static_method(
                "android/content/ClipData",
                "newRawUri",
                "(Ljava/lang/CharSequence;Landroid/net/Uri;)Landroid/content/ClipData;",
                &[JValue::Object(&label), JValue::Object(&uri)],
            )?
            .l()?;
        env.call_method(
            &intent,
            "setClipData",
            "(Landroid/content/ClipData;)V",
            &[JValue::Object(&clip)],
        )?;
        // FLAG_GRANT_READ_URI_PERMISSION.
        env.call_method(
            &intent,
            "addFlags",
            "(I)Landroid/content/Intent;",
            &[JValue::Int(1)],
        )?;
        let heading = JObject::from(env.new_string(title)?);
        let chooser = env
            .call_static_method(
                "android/content/Intent",
                "createChooser",
                "(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;",
                &[JValue::Object(&intent), JValue::Object(&heading)],
            )?
            .l()?;
        env.call_method(
            activity,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::Object(&chooser)],
        )?;
        Ok(())
    })
    .ok_or_else(|| t("No app can receive this file").into())
}

/// The picker's answer, from `documents::document_result` on Android's UI thread.
pub fn document_result(
    mut env: JNIEnv<'_>,
    activity: JObject<'_>,
    request: i32,
    result: i32,
    uri: JString<'_>,
) {
    let text: Option<String> = if uri.is_null() {
        None
    } else {
        env.get_string(&uri).ok().map(Into::into)
    };
    let _ = env.exception_clear();
    let pending = state().pending.take();
    let matches = match &pending {
        Some(Pending::Import { .. } | Pending::Engine { .. } | Pending::Horizon { .. }) => {
            request == IMPORT_WORLD
        }
        Some(Pending::World { .. } | Pending::File { .. }) => request == EXPORT,
        None => false,
    };
    let Some(pending) = pending.filter(|_| matches) else {
        // Android stopped the process while the picker was open: the request
        // is gone. A document created for it would stay empty; remove it.
        if request == EXPORT
            && result == -1
            && let Some(uri) = &text
        {
            crate::document_bridge::delete(&mut env, &activity, uri);
        }
        if request == IMPORT_WORLD || request == EXPORT {
            notify(
                "Transfer interrupted",
                t("Android closed Horizon while the file picker was open. Try again.").into(),
                false,
                false,
            );
        }
        return;
    };
    let Some(uri) = text.filter(|u| result == -1 && u.starts_with("content://") && u.len() <= 8192)
    else {
        if result == -1 {
            notify(
                "Could not open the file",
                t("Android returned an unsupported location.").into(),
                true,
                false,
            );
        }
        // The player closed the picker: nothing happened, nothing to say.
        return;
    };
    let setup = (|| -> jni::errors::Result<_> {
        Ok((env.get_java_vm()?, env.new_global_ref(&activity)?))
    })();
    let Ok((vm, activity)) = setup else {
        let _ = env.exception_clear();
        notify(
            "Could not open the file",
            t("Android did not give access to the chosen file.").into(),
            true,
            false,
        );
        return;
    };
    let (job, name) = match &pending {
        Pending::Import { .. } => (Job::ImportWorld, String::new()),
        Pending::Engine { .. } => (Job::ImportEngine, String::new()),
        Pending::Horizon { .. } => (Job::ImportHorizon, String::new()),
        Pending::World { name, .. } => (Job::ExportWorld, name.clone()),
        Pending::File { path } => (
            Job::SaveFile,
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        ),
    };
    CANCEL.store(false, Ordering::Relaxed);
    state().running = Some(Running {
        job,
        name,
        stage: if matches!(
            job,
            Job::ImportWorld | Job::ImportEngine | Job::ImportHorizon
        ) {
            Stage::Checking
        } else {
            Stage::Writing
        },
        progress: Progress::default(),
        cancelling: false,
    });
    let spawned = std::thread::Builder::new()
        .name("World transfer".into())
        .spawn(move || {
            let outcome = match vm.attach_current_thread() {
                Ok(mut env) => {
                    let outcome = run(&mut env, activity.as_obj(), &uri, pending);
                    let _ = env.exception_clear();
                    // Released while attached, as JNI global references must be.
                    drop(activity);
                    outcome
                }
                Err(error) => Err(Failure::Other(error.to_string())),
            };
            // The notice first: the dialog never closes without its outcome.
            finished(job, outcome);
            state().running = None;
        });
    if let Err(error) = spawned {
        state().running = None;
        finished(job, Err(Failure::Other(error.to_string())));
    }
}

enum Failure {
    Cancelled,
    /// Android could not open the chosen document; why, as Java said it.
    Unreadable(String),
    Archive(archive::Error),
    /// Bytes more the device needs.
    NoSpace(u64),
    Other(String),
}
impl From<archive::Error> for Failure {
    fn from(error: archive::Error) -> Self {
        match error {
            archive::Error::Cancelled => Self::Cancelled,
            archive::Error::Io(error) => error.into(),
            error => Self::Archive(error),
        }
    }
}
impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::StorageFull || error.raw_os_error() == Some(28) {
            Self::NoSpace(0)
        } else {
            Self::Other(error.to_string())
        }
    }
}
impl From<Box<dyn std::error::Error + Send + Sync>> for Failure {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::Other(error.to_string())
    }
}

/// What the player sees when a transfer ends.
fn finished(job: Job, outcome: Result<String, Failure>) {
    let (done, cancelled, failed) = match job {
        Job::ExportWorld => (
            "World exported",
            "Export cancelled",
            "Could not export the world",
        ),
        Job::ImportWorld => (
            "World imported",
            "Import cancelled",
            "Could not import the world",
        ),
        Job::SaveFile => ("File saved", "Saving cancelled", "Could not save the file"),
        Job::ImportEngine => (
            "Engine installed",
            "Engine import cancelled",
            "Could not install the engine",
        ),
        Job::ImportHorizon => (
            "Moved from Horizon Android",
            "Move cancelled",
            "Could not move from Horizon Android",
        ),
    };
    match outcome {
        Ok(name) => notify(
            done,
            match job {
                Job::ImportWorld => tf("«{}» is now in Worlds.", &[&name]),
                Job::ImportEngine => tf(
                    "«{}» is in Settings › Engines. Start it from there.",
                    &[&name],
                ),
                // The summary, already in the launcher's language.
                Job::ImportHorizon => name,
                _ => tf("Saved as «{}».", &[&name]),
            },
            false,
            matches!(job, Job::ImportWorld | Job::ImportHorizon),
        ),
        Err(Failure::Cancelled) => notify(
            cancelled,
            if matches!(
                job,
                Job::ImportWorld | Job::ImportEngine | Job::ImportHorizon
            ) {
                t("Nothing was added.").into()
            } else {
                t("The unfinished file was removed.").into()
            },
            false,
            false,
        ),
        Err(Failure::Unreadable(reason)) => notify(
            failed,
            tf(
                "Android could not open the chosen file ({}). Try it from another place, such as Downloads.",
                &[&reason],
            ),
            true,
            false,
        ),
        Err(Failure::NoSpace(0)) => notify(
            failed,
            t("Not enough free space on the device.").into(),
            true,
            false,
        ),
        Err(Failure::NoSpace(bytes)) => notify(
            failed,
            tf(
                "Not enough free space: free up {} MiB more and try again.",
                &[&bytes.div_ceil(1 << 20).to_string()],
            ),
            true,
            false,
        ),
        Err(Failure::Archive(error)) => notify(failed, t_dyn(&error.to_string()), true, false),
        Err(Failure::Other(error)) => notify(failed, t_dyn(&error), true, false),
    }
}

fn progress(stage: Stage, value: Progress) -> bool {
    if let Some(running) = state().running.as_mut() {
        running.stage = stage;
        running.progress = value;
    }
    !CANCEL.load(Ordering::Relaxed)
}
fn seekable(file: &mut fs::File) -> bool {
    file.stream_position().is_ok()
}
fn free_space(path: &Path) -> Option<u64> {
    rustix::fs::statvfs(path)
        .ok()
        .map(|s| s.f_bavail.saturating_mul(s.f_frsize))
}
/// The work folder beside `saves`, emptied of anything an interrupted transfer
/// left. Refuses a redirected (symbolic link) folder.
fn work_folder(saves: &Path) -> Result<PathBuf, Failure> {
    let home = saves
        .parent()
        .ok_or_else(|| Failure::Other("Invalid profile folder".into()))?;
    let work = home.join(WORK);
    match fs::symlink_metadata(&work) {
        Ok(meta) if meta.file_type().is_symlink() => fs::remove_file(&work)?,
        Ok(_) => fs::remove_dir_all(&work)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::create_dir_all(&work)?;
    Ok(work)
}
/// Copies `input` into `output` a chunk at a time, counting towards `total`.
fn copy(
    input: &mut impl Read,
    output: &mut impl Write,
    stage: Stage,
    total: u64,
) -> Result<u64, Failure> {
    let mut buffer = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    loop {
        if !progress(
            stage,
            Progress {
                bytes: done,
                total_bytes: total,
                ..Progress::default()
            },
        ) {
            return Err(Failure::Cancelled);
        }
        let n = input.read(&mut buffer)?;
        if n == 0 {
            return Ok(done);
        }
        output.write_all(&buffer[..n])?;
        done += n as u64;
    }
}

/// The worker: one transfer from start to finish. Returns the name to report.
fn run(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    uri: &str,
    pending: Pending,
) -> Result<String, Failure> {
    match pending {
        Pending::Import { saves } => {
            let work = work_folder(&saves)?;
            let result = import(env, activity, uri, &saves, &work);
            let _ = fs::remove_dir_all(&work);
            result
        }
        Pending::Engine { home } => import_engine_file(env, activity, uri, &home),
        Pending::Horizon { home, from } => {
            let (name, worlds) = horizon_import::run(env, activity, uri, &home, from)?;
            Ok(horizon_import::summary(&name, worlds))
        }
        Pending::World { dir, .. } => {
            let (mut output, name) = open_created(env, activity, uri, "world.zip")?;
            let result = export(&dir, &mut output);
            let result = result.and_then(|()| Ok(output.sync_all()?));
            drop(output);
            if let Some(saves) = dir.parent() {
                let _ = fs::remove_dir_all(saves.with_file_name(WORK));
            }
            finish_export(env, activity, uri, result, name)
        }
        Pending::File { path } => {
            let (mut output, name) = open_created(env, activity, uri, "report")?;
            let result = (|| -> Result<(), Failure> {
                let mut input = fs::File::open(&path)?;
                let total = input.metadata()?.len();
                copy(&mut input, &mut output, Stage::Writing, total)?;
                Ok(output.sync_all()?)
            })();
            drop(output);
            finish_export(env, activity, uri, result, name)
        }
    }
}
/// Copies the chosen engine file into `files/engines` (a ZIP must be read out of
/// order), then installs it. Returns the engine's name.
fn import_engine_file(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    uri: &str,
    home: &Path,
) -> Result<String, Failure> {
    let (mut input, _) = crate::document_bridge::open(env, activity, uri, "engine.hengine")
        .map_err(|error| unreadable(env, error))?;
    let total = input.metadata().map(|m| m.len()).unwrap_or(0);
    let engines = crate::engines::folder(home);
    fs::create_dir_all(&engines)?;
    if let Some(free) = free_space(&engines)
        && free < total.saturating_mul(2) + MARGIN
    {
        return Err(Failure::NoSpace(total.saturating_mul(2) + MARGIN - free));
    }
    let staged = engines.join(".incoming.hengine");
    let result = (|| -> Result<String, Failure> {
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&staged)?;
        copy(&mut input, &mut file, Stage::Receiving, total)?;
        file.seek(SeekFrom::Start(0))?;
        progress(Stage::Writing, Progress::default());
        crate::engines::install(home, file)
            .map(|m| m.name)
            .map_err(Failure::Other)
    })();
    let _ = fs::remove_file(&staged);
    result
}
/// The document the picker created, opened for writing; removed again when it
/// cannot be opened.
fn open_created(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    uri: &str,
    fallback: &str,
) -> Result<(fs::File, String), Failure> {
    crate::document_bridge::open_write(env, activity, uri, fallback).map_err(|error| {
        let failure = unreadable(env, error);
        crate::document_bridge::delete(env, activity, uri);
        failure
    })
}
/// A document Android could not open, with the reason Java gave.
fn unreadable(env: &mut JNIEnv<'_>, error: Box<dyn std::error::Error + Send + Sync>) -> Failure {
    let reason = crate::document_bridge::take_exception(env).unwrap_or_else(|| error.to_string());
    log::warn!("Document open failed: {reason}");
    Failure::Unreadable(reason)
}
/// A failed or cancelled export leaves no half-written file behind.
fn finish_export(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    uri: &str,
    result: Result<(), Failure>,
    name: String,
) -> Result<String, Failure> {
    if result.is_err() {
        crate::document_bridge::delete(env, activity, uri);
    }
    result.map(|()| name)
}
fn export(dir: &Path, output: &mut fs::File) -> Result<(), Failure> {
    let root = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let entries = archive::world_entries(dir, &root)?;
    let totals = archive::totals(&entries);
    if seekable(output) {
        archive::write_world(&entries, &mut *output, |p| progress(Stage::Writing, p))?;
        return Ok(());
    }
    // A provider that only streams (a pipe): the ZIP needs to seek back to its
    // headers, so it is built in the work folder first, then copied over.
    let saves = dir
        .parent()
        .ok_or_else(|| Failure::Other("Invalid world folder".into()))?;
    let work = work_folder(saves)?;
    let need = totals.total_bytes + MARGIN;
    if let Some(free) = free_space(&work)
        && free < need
    {
        return Err(Failure::NoSpace(need - free));
    }
    let staged = work.join("export.zip");
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&staged)?;
    archive::write_world(&entries, &mut file, |p| {
        // The first half of the bar: packing.
        progress(
            Stage::Writing,
            Progress {
                bytes: p.bytes / 2,
                ..p
            },
        )
    })?;
    let size = file.seek(SeekFrom::End(0))?;
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    loop {
        let copied = done as f64 / size.max(1) as f64;
        let bar = Progress {
            bytes: totals.total_bytes / 2 + (copied * (totals.total_bytes / 2) as f64) as u64,
            files: totals.total_files,
            ..totals
        };
        if !progress(Stage::Writing, bar) {
            return Err(Failure::Cancelled);
        }
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        output.write_all(&buffer[..n])?;
        done += n as u64;
    }
    Ok(())
}
fn import(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    uri: &str,
    saves: &Path,
    work: &Path,
) -> Result<String, Failure> {
    let (mut input, zip_name) = crate::document_bridge::open(env, activity, uri, "world.zip")
        .map_err(|error| unreadable(env, error))?;
    if let Some(running) = state().running.as_mut() {
        running.name = zip_name.clone();
    }
    let file = if seekable(&mut input) {
        input
    } else {
        // Read once from the provider into the work folder, then check it.
        let staged = work.join("import.zip");
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)?;
        copy(&mut input, &mut output, Stage::Receiving, 0)?;
        drop(output);
        fs::File::open(&staged)?
    };
    progress(Stage::Checking, Progress::default());
    let mut zip = zip::ZipArchive::new(file).map_err(archive::Error::from)?;
    let plan = archive::inspect(&mut zip)?;
    let need = plan.bytes + MARGIN;
    if let Some(free) = free_space(work)
        && free < need
    {
        return Err(Failure::NoSpace(need - free));
    }
    let unpacked = work.join("world");
    fs::create_dir(&unpacked)?;
    archive::extract(&mut zip, &plan, &unpacked, |p| progress(Stage::Writing, p))?;
    drop(zip);
    if !unpacked.join("level.dat").is_file() {
        return Err(archive::Error::NoLevelDat.into());
    }
    if fs::symlink_metadata(saves).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Failure::Other("Refusing a redirected saves folder".into()));
    }
    fs::create_dir_all(saves)?;
    let base = archive::import_name(&plan, &zip_name);
    // A world created meanwhile under the same name is never replaced.
    for _ in 0..100 {
        let name = archive::free_name(saves, &base)?;
        match rustix::fs::renameat_with(
            rustix::fs::CWD,
            &unpacked,
            rustix::fs::CWD,
            saves.join(&name),
            rustix::fs::RenameFlags::NOREPLACE,
        ) {
            Ok(()) => return Ok(name),
            Err(rustix::io::Errno::EXIST | rustix::io::Errno::NOTEMPTY) => continue,
            Err(error) => return Err(io::Error::from(error).into()),
        }
    }
    Err(Failure::Other("Too many worlds with this name".into()))
}
