//! Moving over from Horizon Android or Starling (both `dev.horizon.android`, and their
//! `.dev` builds). The player grants the old app's folder in the system folder picker
//! ("Horizon Android"), and what its documents provider shares (worlds, resource packs,
//! shader packs, mods and the configuration files) is copied into a new instance. The
//! old app is only read; nothing in Aurora is overwritten. Accounts are not shared by
//! the old app (Android keeps them to it), so the player signs in again.
//!
//! The old provider's tree is `document_store.rs`'s: the root `horizon` holds
//! `p<profile>/<folder>` for its current profile, with path-like ids below them.
use super::horizon_folders::{ROOT, folders_of, from_horizon, safe_component};
use super::{Failure, MARGIN, Stage, free_space, progress};
use crate::launcher::text::tf;
use crate::world_archive::Progress;
use jni::{
    JNIEnv,
    objects::{JObject, JValue},
};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const DIRECTORY: &str = "vnd.android.document/directory";
/// The most files one move copies.
const MAX_FILES: usize = 200_000;
/// The new instance's name.
pub const INSTANCE: &str = "Horizon Android";

/// A file to copy: its document, and where it goes under the instance's folder.
struct File {
    id: String,
    to: PathBuf,
    size: u64,
}
/// One row of a folder listing.
struct Row {
    id: String,
    name: String,
    directory: bool,
    size: u64,
}

/// Copies the old app's folders in the tree `uri` into a new instance under `home`,
/// with the settings of instance `from` beneath its own. Returns the new instance's
/// name and how many worlds came over.
pub(super) fn run(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    uri: &str,
    home: &Path,
    from: u32,
) -> Result<(String, usize), Failure> {
    if !from_horizon(uri) {
        return Err(Failure::Other(
            crate::launcher::text::t(
                "This folder is not Horizon Android's. Choose «Horizon Android» in the list of places.",
            )
            .into(),
        ));
    }
    let tree = parse(env, uri)?;
    let tree_id = jstring(env, |env| {
        env.call_static_method(
            "android/provider/DocumentsContract",
            "getTreeDocumentId",
            "(Landroid/net/Uri;)Ljava/lang/String;",
            &[JValue::Object(&tree)],
        )
    })?;
    let top: Vec<String> = if tree_id == ROOT {
        list(env, activity, &tree, ROOT)?
            .into_iter()
            .map(|row| row.id)
            .collect()
    } else {
        Vec::new()
    };
    let folders = folders_of(&tree_id, &top).ok_or_else(|| {
        Failure::Other(
            crate::launcher::text::t(
                "Choose Horizon Android's own folder, or one of Worlds, Resource Packs, Shader Packs, Mods and Configuration in it.",
            )
            .into(),
        )
    })?;
    // Everything to copy, before anything is made: a failure leaves no instance.
    let mut files = Vec::new();
    let mut worlds = 0;
    for (id, target) in &folders {
        let base = PathBuf::from(target);
        let mut stack = vec![(id.clone(), base, 0usize)];
        while let Some((folder, to, depth)) = stack.pop() {
            for row in list(env, activity, &tree, &folder)? {
                if !safe_component(&row.name) {
                    continue;
                }
                let path = to.join(&row.name);
                if row.directory {
                    if *target == "saves" && depth == 0 {
                        worlds += 1;
                    }
                    if depth < 64 {
                        stack.push((row.id, path, depth + 1));
                    }
                } else {
                    files.push(File {
                        id: row.id,
                        to: path,
                        size: row.size,
                    });
                }
                if files.len() > MAX_FILES {
                    return Err(Failure::Other("Too many files to move at once".into()));
                }
            }
            if !progress(Stage::Checking, Progress::default()) {
                return Err(Failure::Cancelled);
            }
        }
    }
    let total: u64 = files.iter().map(|f| f.size).sum();
    if let Some(free) = free_space(home)
        && free < total.saturating_add(MARGIN)
    {
        return Err(Failure::NoSpace(total.saturating_add(MARGIN) - free));
    }
    let instance =
        crate::instances::create(home, INSTANCE, None, Some(from)).map_err(Failure::Other)?;
    let folder = crate::instances::folder(home, instance.id);
    let copied = copy_all(env, activity, &tree, &files, &folder, total);
    if let Err(failure) = copied {
        // Nothing half-moved stays behind.
        let _ = crate::instances::delete(home, instance.id);
        return Err(failure);
    }
    Ok((instance.name, worlds))
}

fn copy_all(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    tree: &JObject<'_>,
    files: &[File],
    folder: &Path,
    total: u64,
) -> Result<(), Failure> {
    let mut done = Progress {
        total_bytes: total,
        total_files: files.len(),
        ..Progress::default()
    };
    let mut buffer = vec![0u8; 256 << 10];
    for file in files {
        if !progress(Stage::Writing, done) {
            return Err(Failure::Cancelled);
        }
        let uri = jstring(env, |env| {
            let id = JObject::from(env.new_string(&file.id)?);
            let uri = env
                .call_static_method(
                    "android/provider/DocumentsContract",
                    "buildDocumentUriUsingTree",
                    "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
                    &[JValue::Object(tree), JValue::Object(&id)],
                )?
                .l()?;
            env.call_method(&uri, "toString", "()Ljava/lang/String;", &[])
        })?;
        let (mut input, _) = crate::document_bridge::open(env, activity, &uri, "")
            .map_err(|error| unshared(&error.to_string()))?;
        let path = folder.join(&file.to);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::File::create(&path)?;
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            output.write_all(&buffer[..n])?;
            done.bytes += n as u64;
            if !progress(Stage::Writing, done) {
                return Err(Failure::Cancelled);
            }
        }
        output.sync_all()?;
        done.files += 1;
    }
    Ok(())
}

/// `Uri.parse(uri)`.
fn parse<'a>(env: &mut JNIEnv<'a>, uri: &str) -> Result<JObject<'a>, Failure> {
    let result = (|| {
        let text = JObject::from(env.new_string(uri)?);
        env.call_static_method(
            "android/net/Uri",
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(&text)],
        )?
        .l()
    })();
    result.map_err(|error| java_failure(env, error))
}

/// A Java call that returns a `String`, as Rust text.
fn jstring(
    env: &mut JNIEnv<'_>,
    call: impl for<'l> FnOnce(&mut JNIEnv<'l>) -> jni::errors::Result<jni::objects::JValueOwned<'l>>,
) -> Result<String, Failure> {
    let result = env.with_local_frame(8, |env| -> jni::errors::Result<String> {
        let value = call(env)?.l()?;
        if value.is_null() {
            return Ok(String::new());
        }
        Ok(env.get_string((&value).into())?.into())
    });
    result.map_err(|error| java_failure(env, error))
}

/// The children of document `id` in `tree`, as the old provider lists them.
fn list(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    tree: &JObject<'_>,
    id: &str,
) -> Result<Vec<Row>, Failure> {
    let result = env.with_local_frame(32, |env| -> jni::errors::Result<Vec<Row>> {
        let resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )?
            .l()?;
        let id = JObject::from(env.new_string(id)?);
        let children = env
            .call_static_method(
                "android/provider/DocumentsContract",
                "buildChildDocumentsUriUsingTree",
                "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(tree), JValue::Object(&id)],
            )?
            .l()?;
        let projection = env.new_object_array(4, "java/lang/String", JObject::null())?;
        for (i, column) in ["document_id", "_display_name", "mime_type", "_size"]
            .iter()
            .enumerate()
        {
            let column = env.new_string(column)?;
            env.set_object_array_element(&projection, i as i32, column)?;
        }
        let null = JObject::null();
        let cursor = env
            .call_method(
                &resolver,
                "query",
                "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                &[
                    JValue::Object(&children),
                    JValue::Object(projection.as_ref()),
                    JValue::Object(&null),
                    JValue::Object(&null),
                    JValue::Object(&null),
                ],
            )?
            .l()?;
        if cursor.is_null() {
            return Ok(Vec::new());
        }
        let rows = (|| -> jni::errors::Result<Vec<Row>> {
            let mut rows = Vec::new();
            while env.call_method(&cursor, "moveToNext", "()Z", &[])?.z()? {
                let row = env.with_local_frame(8, |env| -> jni::errors::Result<Row> {
                    let text = |env: &mut JNIEnv<'_>, column: i32| -> jni::errors::Result<String> {
                        let value = env
                            .call_method(&cursor, "getString", "(I)Ljava/lang/String;", &[JValue::Int(column)])?
                            .l()?;
                        if value.is_null() {
                            return Ok(String::new());
                        }
                        Ok(env.get_string((&value).into())?.into())
                    };
                    let id = text(env, 0)?;
                    let name = text(env, 1)?;
                    let mime = text(env, 2)?;
                    let size = env
                        .call_method(&cursor, "getLong", "(I)J", &[JValue::Int(3)])?
                        .j()?;
                    Ok(Row {
                        id,
                        name,
                        directory: mime == DIRECTORY,
                        size: u64::try_from(size).unwrap_or(0),
                    })
                })?;
                rows.push(row);
            }
            Ok(rows)
        })();
        env.call_method(&cursor, "close", "()V", &[])?;
        rows
    });
    result.map_err(|error| java_failure(env, error))
}

/// A failed Java call as the player reads it: Java's own reason when it threw (the old
/// provider refuses worlds while its game runs, for one).
fn java_failure(env: &mut JNIEnv<'_>, error: jni::errors::Error) -> Failure {
    let reason = match error {
        jni::errors::Error::JavaException => {
            crate::document_bridge::take_exception(env).unwrap_or_else(|| "Java".into())
        }
        error => error.to_string(),
    };
    unshared(&reason)
}
fn unshared(reason: &str) -> Failure {
    Failure::Other(tf(
        "Horizon Android could not share its files: {}. Close its game and try again.",
        &[reason],
    ))
}

/// What the notice says once the move is done.
pub(super) fn summary(name: &str, worlds: usize) -> String {
    tf(
        "Instance «{}»: {} world(s). Choose it on Home or under Instances, and sign in to your accounts again.",
        &[name, &worlds.to_string()],
    )
}
