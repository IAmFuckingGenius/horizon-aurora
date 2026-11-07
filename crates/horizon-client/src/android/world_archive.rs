//! World ZIPs for the launcher: a streamed archive of one world folder, and a
//! checked unpack of an archive into a fresh folder. No Android here, so host
//! tests cover zip-slip, symbolic links, name collisions and a missing level.dat
//! (`tools/test_android_worlds.py`).
use std::{
    collections::HashSet,
    fmt, fs,
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// The lock a running game holds on its world. Never archived, never unpacked.
pub const SESSION_LOCK: &str = "session.lock";
/// More files than any world has; a ZIP listing more is refused.
const MAX_ENTRIES: usize = 200_000;
/// The longest folder name an imported world gets, in characters.
const MAX_NAME: usize = 80;
const CHUNK: usize = 64 * 1024;

/// Bytes and files done of the whole, for a progress bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub bytes: u64,
    pub total_bytes: u64,
    pub files: usize,
    pub total_files: usize,
}

#[derive(Debug)]
pub enum Error {
    Cancelled,
    Io(io::Error),
    NotZip,
    NoLevelDat,
    SeveralWorlds,
    UnsafePath,
    Symlink,
    Duplicate,
    Encrypted,
    Unsupported,
    TooManyFiles,
    SizeMismatch,
}
impl fmt::Display for Error {
    /// English sentences; the launcher's text table has them in Russian.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Cancelled",
            Self::Io(error) => return write!(f, "{error}"),
            Self::NotZip => "The file is not a ZIP archive, or it is damaged.",
            Self::NoLevelDat => {
                "No world in this ZIP: level.dat must be at its root or in one folder inside it."
            }
            Self::SeveralWorlds => {
                "The ZIP holds more than one world. Put each world in its own ZIP."
            }
            Self::UnsafePath => {
                "The ZIP has a file path that leads outside the world folder, so nothing was unpacked."
            }
            Self::Symlink => {
                "The ZIP has a symbolic link. Worlds never contain them, so nothing was unpacked."
            }
            Self::Duplicate => "The ZIP lists the same file twice, so nothing was unpacked.",
            Self::Encrypted => "The ZIP is password-protected. Pack the world without a password.",
            Self::Unsupported => {
                "The ZIP uses a compression method Horizon cannot read. Pack it as a regular ZIP."
            }
            Self::TooManyFiles => "The ZIP has too many files to be one world.",
            Self::SizeMismatch => "The ZIP is damaged: a file is not the size it claims.",
        })
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<zip::result::ZipError> for Error {
    fn from(error: zip::result::ZipError) -> Self {
        use zip::result::ZipError;
        match error {
            ZipError::Io(error) => Self::Io(error),
            ZipError::UnsupportedArchive(_) => Self::Unsupported,
            _ => Self::NotZip,
        }
    }
}

// ---- export ----

/// One file or folder of a world, as it goes into the ZIP.
#[derive(Debug)]
pub struct Entry {
    pub path: PathBuf,
    /// The path in the ZIP: `root/…`, with `/` after a folder.
    pub name: String,
    pub size: u64,
    pub modified: Option<SystemTime>,
}
impl Entry {
    fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }
}

/// Every folder and regular file of `world`, under `root/` in the ZIP, sorted.
/// Leaves out the world's session.lock and never follows a symbolic link.
pub fn world_entries(world: &Path, root: &str) -> io::Result<Vec<Entry>> {
    fn walk(dir: &Path, prefix: &str, top: bool, out: &mut Vec<Entry>) -> io::Result<()> {
        let mut children: Vec<_> = fs::read_dir(dir)?.collect::<io::Result<_>>()?;
        children.sort_by_key(|e| e.file_name());
        for child in children {
            let name = child.file_name().to_string_lossy().into_owned();
            if top && name == SESSION_LOCK {
                continue;
            }
            let meta = fs::symlink_metadata(child.path())?;
            if meta.is_dir() {
                let folder = format!("{prefix}{name}/");
                out.push(Entry {
                    path: child.path(),
                    name: folder.clone(),
                    size: 0,
                    modified: meta.modified().ok(),
                });
                if out.len() > MAX_ENTRIES {
                    return Err(io::Error::other("The world has too many files for one ZIP"));
                }
                walk(&child.path(), &folder, false, out)?;
            } else if meta.is_file() {
                out.push(Entry {
                    path: child.path(),
                    name: format!("{prefix}{name}"),
                    size: meta.len(),
                    modified: meta.modified().ok(),
                });
            }
            // Symbolic links, sockets and devices stay out of the ZIP.
        }
        Ok(())
    }
    let meta = fs::symlink_metadata(world)?;
    if !meta.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The world folder is missing",
        ));
    }
    let root = format!("{}/", root.trim_matches('/'));
    let mut out = vec![Entry {
        path: world.to_owned(),
        name: root.clone(),
        size: 0,
        modified: meta.modified().ok(),
    }];
    walk(world, &root, true, &mut out)?;
    Ok(out)
}

/// The totals a progress bar counts towards.
pub fn totals(entries: &[Entry]) -> Progress {
    let files = entries.iter().filter(|e| !e.is_dir());
    Progress {
        total_bytes: files.clone().map(|e| e.size).sum(),
        total_files: files.count(),
        ..Progress::default()
    }
}

/// Writes `entries` as a ZIP into `out`, a file at a time, never holding one in
/// memory. `progress` sees every 64 KiB; returning false cancels.
pub fn write_world<W: Write + Seek>(
    entries: &[Entry],
    out: W,
    mut progress: impl FnMut(Progress) -> bool,
) -> Result<W, Error> {
    let mut zip = zip::ZipWriter::new(out);
    let mut done = totals(entries);
    let mut buffer = vec![0u8; CHUNK];
    for entry in entries {
        let options = zip::write::SimpleFileOptions::default()
            .last_modified_time(entry.modified.map(dos_time).unwrap_or_default());
        if entry.is_dir() {
            zip.add_directory(entry.name.trim_end_matches('/'), options)?;
            continue;
        }
        // Region files are mostly compressed chunks: a fast level is enough.
        let options = options
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(if entry.size > 1 << 20 { Some(1) } else { None })
            .large_file(entry.size >= u64::from(u32::MAX));
        zip.start_file(entry.name.as_str(), options)?;
        let mut input = fs::File::open(&entry.path)?;
        loop {
            if !progress(done) {
                return Err(Error::Cancelled);
            }
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            zip.write_all(&buffer[..n])?;
            done.bytes += n as u64;
        }
        done.files += 1;
    }
    let out = zip.finish()?;
    progress(done);
    Ok(out)
}

/// A file time as the ZIP's DOS date (UTC, two-second steps, 1980 to 2107).
fn dos_time(time: SystemTime) -> zip::DateTime {
    let Ok(since) = time.duration_since(UNIX_EPOCH) else {
        return zip::DateTime::default();
    };
    let secs = since.as_secs();
    let (days, rest) = ((secs / 86_400) as i64, secs % 86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let Ok(year) = u16::try_from(year) else {
        return zip::DateTime::default();
    };
    zip::DateTime::from_date_and_time(
        year,
        month as u8,
        day as u8,
        (rest / 3600) as u8,
        (rest / 60 % 60) as u8,
        (rest % 60) as u8 & !1,
    )
    .unwrap_or_default()
}

/// A world's name as a file name: `<name>.zip`, without the characters file
/// systems refuse.
pub fn export_file_name(world_name: &str) -> String {
    format!("{}.zip", sanitize(world_name))
}

// ---- import ----

/// Where the world sits in a checked ZIP, and how much it unpacks to.
#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    /// `""` when level.dat is at the root, else the one folder holding it, with `/`.
    pub prefix: String,
    pub files: usize,
    pub bytes: u64,
}
impl Plan {
    /// The ZIP's own world folder, when the world is one folder down.
    pub fn folder(&self) -> Option<&str> {
        self.prefix.strip_suffix('/').filter(|s| !s.is_empty())
    }
}

/// True when `name` is a relative path that stays inside the folder it is
/// unpacked to: no absolute path, drive, `..`, `.`, empty part, backslash,
/// colon or control character.
pub fn safe_name(name: &str) -> bool {
    let trimmed = name.strip_suffix('/').unwrap_or(name);
    !trimmed.is_empty()
        && name.len() <= 1024
        && !name.starts_with('/')
        && !name.contains(['\\', ':', '\0'])
        && !name.chars().any(char::is_control)
        && trimmed
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Checks every entry of `zip` before anything is unpacked, and finds the world:
/// level.dat at the root, or in exactly one folder one level down.
pub fn inspect<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> Result<Plan, Error> {
    if zip.len() > MAX_ENTRIES {
        return Err(Error::TooManyFiles);
    }
    let mut names = HashSet::new();
    let mut root = false;
    let mut folders = Vec::<String>::new();
    let mut entries = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let entry = zip.by_index_raw(i)?;
        let name = entry.name()?.into_owned();
        if !safe_name(&name) || entry.enclosed_name().is_none() {
            return Err(Error::UnsafePath);
        }
        if entry.is_symlink() || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err(Error::Symlink);
        }
        if entry.encrypted() {
            return Err(Error::Encrypted);
        }
        if !matches!(
            entry.compression(),
            zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
        ) {
            return Err(Error::Unsupported);
        }
        let dir = entry.is_dir();
        let key = name.trim_end_matches('/').to_owned();
        if !names.insert(key.clone()) {
            return Err(Error::Duplicate);
        }
        if !dir {
            let parts: Vec<&str> = key.split('/').collect();
            match parts.as_slice() {
                ["level.dat"] => root = true,
                [folder, "level.dat"] if !folders.iter().any(|f| f == folder) => {
                    folders.push((*folder).to_owned())
                }
                _ => {}
            }
        }
        entries.push((key, dir, entry.size()));
    }
    let prefix = if root {
        String::new()
    } else {
        match folders.as_slice() {
            [] => return Err(Error::NoLevelDat),
            [folder] => format!("{folder}/"),
            _ => return Err(Error::SeveralWorlds),
        }
    };
    let mut plan = Plan {
        prefix,
        files: 0,
        bytes: 0,
    };
    for (key, dir, size) in entries {
        if let Some(rel) = relative(&key, &plan.prefix)
            && !dir
            && rel != SESSION_LOCK
        {
            plan.files += 1;
            plan.bytes = plan.bytes.saturating_add(size);
        }
    }
    Ok(plan)
}

/// `key` inside the world (`prefix` taken off), or None for a file outside it
/// (another folder's `__MACOSX/…`) and for the world folder itself.
fn relative<'a>(key: &'a str, prefix: &str) -> Option<&'a str> {
    key.strip_prefix(prefix).filter(|rel| !rel.is_empty())
}

/// Unpacks the world `plan` found into `target`, an empty folder the caller
/// made. Files are created new (never through an existing path) and may not
/// grow past the size the ZIP claims. `progress` sees every 64 KiB; returning
/// false cancels. The caller removes `target` on an error.
pub fn extract<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    plan: &Plan,
    target: &Path,
    mut progress: impl FnMut(Progress) -> bool,
) -> Result<(), Error> {
    let mut done = Progress {
        total_bytes: plan.bytes,
        total_files: plan.files,
        ..Progress::default()
    };
    let mut buffer = vec![0u8; CHUNK];
    for i in 0..zip.len() {
        if !progress(done) {
            return Err(Error::Cancelled);
        }
        let mut entry = zip.by_index(i)?;
        let name = entry.name()?.into_owned();
        // `inspect` checked these; check again where the path is built.
        if !safe_name(&name) || entry.is_symlink() {
            return Err(Error::UnsafePath);
        }
        let key = name.trim_end_matches('/');
        let Some(rel) = relative(key, &plan.prefix) else {
            continue;
        };
        if rel == SESSION_LOCK {
            continue;
        }
        let mut path = target.to_owned();
        for part in rel.split('/') {
            path.push(part);
        }
        if !path.starts_with(target) {
            return Err(Error::UnsafePath);
        }
        if entry.is_dir() {
            fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let expected = entry.size();
        let mut written = 0u64;
        loop {
            if !progress(done) {
                return Err(Error::Cancelled);
            }
            let n = entry.read(&mut buffer).map_err(|e| {
                if e.kind() == io::ErrorKind::InvalidData {
                    Error::SizeMismatch
                } else {
                    Error::Io(e)
                }
            })?;
            if n == 0 {
                break;
            }
            written += n as u64;
            if written > expected {
                return Err(Error::SizeMismatch);
            }
            output.write_all(&buffer[..n])?;
            done.bytes += n as u64;
        }
        if written != expected {
            return Err(Error::SizeMismatch);
        }
        done.files += 1;
    }
    progress(done);
    Ok(())
}

/// A folder or file name without the characters Android, Windows and macOS
/// refuse, trimmed, never hidden, at most 80 characters, "World" when nothing
/// is left.
pub fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .take(MAX_NAME)
        .collect();
    let cleaned = cleaned
        .trim()
        .trim_start_matches('.')
        .trim_end_matches(['.', ' '])
        .trim();
    if cleaned.is_empty() {
        "World".into()
    } else {
        cleaned.into()
    }
}

/// The folder an imported world gets: the ZIP's own world folder, else the ZIP's
/// file name without `.zip`.
pub fn import_name(plan: &Plan, zip_name: &str) -> String {
    let stem = zip_name
        .rsplit_once('.')
        .filter(|(_, ext)| ext.eq_ignore_ascii_case("zip"))
        .map_or(zip_name, |(stem, _)| stem);
    sanitize(plan.folder().unwrap_or(stem))
}

/// `name`, or the first of "name (2)", "name (3)"… with no file or folder in
/// `saves`. A name already ending in " (n)" counts on from n.
pub fn free_name(saves: &Path, name: &str) -> io::Result<String> {
    let name = sanitize(name);
    let taken = |candidate: &str| fs::symlink_metadata(saves.join(candidate)).is_ok();
    if !taken(&name) {
        return Ok(name);
    }
    let (stem, first) = name
        .strip_suffix(')')
        .and_then(|s| s.rsplit_once(" ("))
        .and_then(|(stem, n)| Some((stem.to_owned(), n.parse::<u32>().ok()? + 1)))
        .filter(|(stem, _)| !stem.trim().is_empty())
        .unwrap_or((name.clone(), 2));
    for n in first..first.saturating_add(10_000) {
        let candidate = format!("{stem} ({n})");
        if !taken(&candidate) {
            return Ok(candidate);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Too many worlds with this name",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "horizon-world-archive-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
    fn zip_of(entries: &[(&str, &[u8])]) -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            if let Some(dir) = name.strip_suffix('/') {
                writer
                    .add_directory(dir, zip::write::SimpleFileOptions::default())
                    .unwrap();
            } else {
                writer
                    .start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(bytes).unwrap();
            }
        }
        zip::ZipArchive::new(writer.finish().unwrap()).unwrap()
    }

    #[test]
    fn export_streams_the_folder_without_the_lock_or_links_and_unpacks_back() {
        let dir = temp("export");
        let world = dir.join("My World");
        fs::create_dir_all(world.join("region")).unwrap();
        fs::create_dir_all(world.join("datapacks")).unwrap();
        fs::write(world.join("level.dat"), b"level").unwrap();
        fs::write(world.join(SESSION_LOCK), b"lock").unwrap();
        // Larger than one copy chunk, so progress runs inside a file.
        let region: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        fs::write(world.join("region/r.0.0.mca"), &region).unwrap();
        // A nested session.lock is an ordinary file; only the world's own is skipped.
        fs::write(world.join("region/session.lock"), b"kept").unwrap();
        std::os::unix::fs::symlink(dir.join("outside"), world.join("link")).unwrap();

        let entries = world_entries(&world, "My World").unwrap();
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "My World/",
                "My World/datapacks/",
                "My World/level.dat",
                "My World/region/",
                "My World/region/r.0.0.mca",
                "My World/region/session.lock",
            ]
        );
        let mut seen = Vec::new();
        let bytes = write_world(&entries, Cursor::new(Vec::new()), |p| {
            seen.push(p);
            true
        })
        .unwrap()
        .into_inner();
        let last = *seen.last().unwrap();
        assert_eq!(last.total_files, 3);
        assert_eq!(last.files, 3);
        assert_eq!(last.bytes, 5 + 200_000 + 4);
        assert_eq!(last.bytes, last.total_bytes);
        assert!(seen.windows(2).all(|w| w[0].bytes <= w[1].bytes));

        // The export is a valid world import, one folder down.
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let plan = inspect(&mut zip).unwrap();
        assert_eq!(plan.prefix, "My World/");
        assert_eq!(plan.files, 3);
        let target = dir.join("unpacked");
        fs::create_dir(&target).unwrap();
        extract(&mut zip, &plan, &target, |_| true).unwrap();
        assert_eq!(fs::read(target.join("level.dat")).unwrap(), b"level");
        assert_eq!(fs::read(target.join("region/r.0.0.mca")).unwrap(), region);
        assert!(target.join("datapacks").is_dir());
        assert!(!target.join(SESSION_LOCK).exists());
        assert!(!target.join("link").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn export_stops_when_cancelled() {
        let dir = temp("cancel");
        fs::write(dir.join("level.dat"), vec![7u8; 300_000]).unwrap();
        let entries = world_entries(&dir, "W").unwrap();
        let mut calls = 0;
        let result = write_world(&entries, Cursor::new(Vec::new()), |_| {
            calls += 1;
            calls < 3
        });
        assert!(matches!(result, Err(Error::Cancelled)));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn level_dat_at_the_root_or_one_folder_down() {
        let root = inspect(&mut zip_of(&[
            ("level.dat", b"l"),
            ("region/r.0.0.mca", b"rr"),
            ("session.lock", b"x"),
        ]))
        .unwrap();
        assert_eq!(
            root,
            Plan {
                prefix: String::new(),
                files: 2,
                bytes: 3
            }
        );
        assert_eq!(root.folder(), None);
        // macOS adds __MACOSX beside the world; it stays out of the plan.
        let nested = inspect(&mut zip_of(&[
            ("Survival/", b""),
            ("Survival/level.dat", b"l"),
            ("__MACOSX/Survival/._level.dat", b"junk"),
        ]))
        .unwrap();
        assert_eq!(nested.folder(), Some("Survival"));
        assert_eq!(nested.files, 1);
        for missing in [
            &[("region/r.0.0.mca", &b"r"[..])][..],
            &[("a/b/level.dat", &b"l"[..])][..],
            &[("level.dat/", &b""[..])][..],
        ] {
            assert!(
                matches!(inspect(&mut zip_of(missing)), Err(Error::NoLevelDat)),
                "{missing:?}"
            );
        }
        assert!(matches!(
            inspect(&mut zip_of(&[("A/level.dat", b"1"), ("B/level.dat", b"2")])),
            Err(Error::SeveralWorlds)
        ));
        let not_zip = zip::ZipArchive::new(Cursor::new(b"not a zip".to_vec()));
        assert!(matches!(not_zip.map_err(Error::from), Err(Error::NotZip)));
    }

    #[test]
    fn unsafe_paths_links_and_duplicates_are_refused_before_unpacking() {
        for name in [
            "../level.dat",
            "World/../../evil",
            "/level.dat",
            "C:/level.dat",
            "World\\..\\evil",
            "World//level.dat",
            "./level.dat",
            "World/\u{7}bell",
        ] {
            let result = inspect(&mut zip_of(&[("level.dat", b"l"), (name, b"x")]));
            assert!(matches!(result, Err(Error::UnsafePath)), "{name}");
        }
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("level.dat", options).unwrap();
        writer.add_symlink("region", "/etc", options).unwrap();
        let mut zip = zip::ZipArchive::new(writer.finish().unwrap()).unwrap();
        assert!(matches!(inspect(&mut zip), Err(Error::Symlink)));
        // The writer refuses a second "level.dat"; rename "level.daX" in the bytes.
        let mut bytes = zip_of(&[("level.dat", b"1"), ("level.daX", b"2")]).into_inner();
        let bytes = bytes.get_mut();
        for i in 0..bytes.len() - 9 {
            if &bytes[i..i + 9] == b"level.daX" {
                bytes[i + 8] = b't';
            }
        }
        // zip 2 keeps one entry per name; either way a name is never unpacked twice.
        let duplicate = zip::ZipArchive::new(Cursor::new(bytes.clone()))
            .map_err(Error::from)
            .and_then(|mut zip| inspect(&mut zip));
        assert!(
            matches!(duplicate, Err(Error::Duplicate | Error::NotZip))
                || duplicate.as_ref().is_ok_and(|plan| plan.files == 1),
            "{duplicate:?}"
        );
        assert!(safe_name("World/region/r.0.0.mca"));
        assert!(safe_name("World/"));
    }

    #[test]
    fn unpacking_never_writes_outside_the_target() {
        let dir = temp("slip");
        let target = dir.join("target");
        fs::create_dir(&target).unwrap();
        // A plan made for a different, safe ZIP must not let a hostile one through.
        let plan = Plan {
            prefix: String::new(),
            files: 1,
            bytes: 1,
        };
        let mut zip = zip_of(&[("level.dat", b"l"), ("../escape", b"x")]);
        assert!(matches!(
            extract(&mut zip, &plan, &target, |_| true),
            Err(Error::UnsafePath)
        ));
        assert!(!dir.join("escape").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn names_are_sanitized_and_collisions_get_a_free_number() {
        let saves = temp("names");
        assert_eq!(free_name(&saves, "World").unwrap(), "World");
        fs::create_dir(saves.join("World")).unwrap();
        assert_eq!(free_name(&saves, "World").unwrap(), "World (2)");
        fs::create_dir(saves.join("World (2)")).unwrap();
        // A plain file takes the name too.
        fs::write(saves.join("World (3)"), b"").unwrap();
        assert_eq!(free_name(&saves, "World").unwrap(), "World (4)");
        assert_eq!(free_name(&saves, "World (2)").unwrap(), "World (4)");
        assert_eq!(free_name(&saves, "Fresh (7)").unwrap(), "Fresh (7)");
        fs::remove_dir_all(saves).unwrap();

        assert_eq!(sanitize("  ../a/b:c*?.  "), "_a_b_c__");
        assert_eq!(sanitize(".hidden"), "hidden");
        assert_eq!(sanitize("..."), "World");
        assert_eq!(sanitize("Мир\u{0}\n"), "Мир");
        assert_eq!(sanitize(&"x".repeat(200)).chars().count(), 80);
        assert_eq!(export_file_name("Новый мир: 2"), "Новый мир_ 2.zip");

        let root = Plan {
            prefix: String::new(),
            files: 1,
            bytes: 1,
        };
        assert_eq!(import_name(&root, "Backup.ZIP"), "Backup");
        assert_eq!(import_name(&root, "world.tar"), "world.tar");
        let nested = Plan {
            prefix: "Creative World/".into(),
            ..root
        };
        assert_eq!(import_name(&nested, "download (1).zip"), "Creative World");
    }

    #[test]
    fn dos_times_are_utc_dates() {
        let time = UNIX_EPOCH + std::time::Duration::from_secs(1_762_172_645);
        let dos = dos_time(time);
        // 2025-11-03 12:24:05 UTC, seconds in two-second steps.
        assert_eq!(
            (
                dos.year(),
                dos.month(),
                dos.day(),
                dos.hour(),
                dos.minute(),
                dos.second()
            ),
            (2025, 11, 3, 12, 24, 4)
        );
        assert_eq!(dos_time(UNIX_EPOCH).year(), 1980);
    }
}
