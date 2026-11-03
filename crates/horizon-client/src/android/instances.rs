//! Instances: what the player plays, each a name, an engine (`engines.rs`; none is the
//! APK's own) and its own game folder (worlds, settings, add-ons). An instance is a
//! launcher profile with a name and an engine: instance 0 is the main game folder,
//! instance N lives in `profiles/profile-N`. Their names and engines are kept in
//! `instances.json`; a profile folder without an entry is an instance with its old
//! name, so nothing a player had before goes missing.
use std::{
    fs,
    path::{Path, PathBuf},
};

/// The file of names and engines, in the launcher's home.
const INDEX: &str = "instances.json";
/// The settings a new instance starts with from the one in use: language, controls,
/// video and Horizon's graphics and extras. Worlds, add-ons and the skin stay its own.
const SETTINGS: [&str; 4] = [
    "options.txt",
    "horizon-touch.json",
    "horizon-graphics.txt",
    "horizon-extras.txt",
];
/// The longest name kept, in characters.
pub const NAME_LIMIT: usize = 32;

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Instance {
    /// The profile number: 0 is the main folder.
    pub id: u32,
    /// Empty: the default name (`Main`, `Profile N`), in the launcher's language.
    #[serde(default)]
    pub name: String,
    /// The installed engine it runs on; `None`: the APK's own.
    #[serde(default)]
    pub engine: Option<String>,
    /// When it was last played, in seconds since 1970.
    #[serde(default)]
    pub played: Option<u64>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Index {
    #[serde(default)]
    instances: Vec<Instance>,
}

/// The game folder of instance `id`.
pub fn folder(home: &Path, id: u32) -> PathBuf {
    if id == 0 {
        home.into()
    } else {
        home.join("profiles").join(format!("profile-{id}"))
    }
}

fn read(home: &Path) -> Index {
    fs::read(home.join(INDEX))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn write(home: &Path, index: &Index) -> Result<(), String> {
    let partial = home.join(".instances.partial");
    fs::write(
        &partial,
        serde_json::to_vec_pretty(index).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(&partial, home.join(INDEX)).map_err(|e| e.to_string())
}
/// The profile numbers that have a folder, with 0 always.
fn profile_ids(home: &Path) -> Vec<u32> {
    let mut ids = vec![0];
    ids.extend(
        fs::read_dir(home.join("profiles"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                e.file_name()
                    .to_str()?
                    .strip_prefix("profile-")?
                    .parse::<u32>()
                    .ok()
            }),
    );
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Every instance: the main one first, then by number.
pub fn list(home: &Path) -> Vec<Instance> {
    let index = read(home);
    profile_ids(home)
        .into_iter()
        .map(|id| {
            index
                .instances
                .iter()
                .find(|i| i.id == id)
                .cloned()
                .unwrap_or(Instance {
                    id,
                    ..Instance::default()
                })
        })
        .collect()
}
/// Instance `id`, as `list` has it.
pub fn get(home: &Path, id: u32) -> Instance {
    list(home)
        .into_iter()
        .find(|i| i.id == id)
        .unwrap_or(Instance {
            id,
            ..Instance::default()
        })
}

/// A name as kept: single-line, trimmed, at most [`NAME_LIMIT`] characters.
pub fn clean_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(NAME_LIMIT)
        .collect()
}

/// Saves `instance` (its name cleaned), creating its folder.
pub fn save(home: &Path, instance: &Instance) -> Result<(), String> {
    fs::create_dir_all(folder(home, instance.id)).map_err(|e| e.to_string())?;
    let mut index = read(home);
    let mut instance = instance.clone();
    instance.name = clean_name(&instance.name);
    index.instances.retain(|i| i.id != instance.id);
    index.instances.push(instance);
    index.instances.sort_by_key(|i| i.id);
    write(home, &index)
}

/// A new instance after the highest number in use, with its folder and the settings
/// of instance `from` (the one in use), so it starts in the player's language and
/// with their controls rather than as a fresh install.
pub fn create(
    home: &Path,
    name: &str,
    engine: Option<String>,
    from: Option<u32>,
) -> Result<Instance, String> {
    let used = profile_ids(home)
        .into_iter()
        .chain(read(home).instances.iter().map(|i| i.id))
        .max()
        .unwrap_or(0);
    let instance = Instance {
        id: used.checked_add(1).ok_or("Too many instances")?,
        name: name.into(),
        engine,
        played: None,
    };
    save(home, &instance)?;
    if let Some(from) = from {
        let (source, target) = (folder(home, from), folder(home, instance.id));
        for file in SETTINGS {
            if source.join(file).is_file() {
                fs::copy(source.join(file), target.join(file)).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(get(home, instance.id))
}

/// Deletes instance `id` with its whole folder (worlds included). The main instance
/// stays: its folder is the launcher's own.
pub fn delete(home: &Path, id: u32) -> Result<(), String> {
    if id == 0 {
        return Err("The main instance cannot be deleted.".into());
    }
    let dir = folder(home, id);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    let mut index = read(home);
    index.instances.retain(|i| i.id != id);
    write(home, &index)
}

/// Records that instance `id` is being played now.
pub fn played(home: &Path, id: u32, now: u64) -> Result<(), String> {
    let mut instance = get(home, id);
    instance.played = Some(now);
    save(home, &instance)
}

/// An engine left the device: instances on it go back to the APK's engine.
pub fn forget_engine(home: &Path, engine: &str) -> Result<(), String> {
    let mut index = read(home);
    let mut changed = false;
    for instance in &mut index.instances {
        if instance.engine.as_deref() == Some(engine) {
            instance.engine = None;
            changed = true;
        }
    }
    if changed { write(home, &index) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("horizon-instances-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn old_profiles_are_instances_and_new_ones_take_the_next_number() {
        let home = home("list");
        fs::create_dir_all(home.join("profiles/profile-3")).unwrap();
        let ids: Vec<u32> = list(&home).iter().map(|i| i.id).collect();
        assert_eq!(ids, [0, 3]);
        assert!(
            list(&home)
                .iter()
                .all(|i| i.name.is_empty() && i.engine.is_none())
        );
        fs::write(home.join("options.txt"), "lang:ru_ru\n").unwrap();
        fs::create_dir_all(home.join("saves/W")).unwrap();
        let pvp = create(&home, "  PvP\n сборка ", Some("pvp-1".into()), Some(0)).unwrap();
        // It starts with the main instance's settings, not its worlds.
        assert_eq!(
            fs::read_to_string(folder(&home, 4).join("options.txt")).unwrap(),
            "lang:ru_ru\n"
        );
        assert!(!folder(&home, 4).join("saves").exists());
        assert_eq!((pvp.id, pvp.name.as_str()), (4, "PvP сборка"));
        assert!(folder(&home, 4).is_dir());
        assert_eq!(get(&home, 4).engine.as_deref(), Some("pvp-1"));
        // Renaming keeps the engine; the main instance can be named too.
        let mut main = get(&home, 0);
        main.name = "Ванилла".into();
        save(&home, &main).unwrap();
        assert_eq!(list(&home)[0].name, "Ванилла");
        played(&home, 4, 1_700_000_000).unwrap();
        assert_eq!(get(&home, 4).played, Some(1_700_000_000));
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn deleting_takes_the_folder_but_never_the_main_one_and_engines_fall_back() {
        let home = home("delete");
        let a = create(&home, "A", Some("e1".into()), None).unwrap();
        let b = create(&home, "B", Some("e1".into()), None).unwrap();
        fs::create_dir_all(folder(&home, a.id).join("saves/W")).unwrap();
        delete(&home, a.id).unwrap();
        assert!(!folder(&home, a.id).exists());
        assert_eq!(
            list(&home).iter().map(|i| i.id).collect::<Vec<_>>(),
            [0, b.id]
        );
        assert!(delete(&home, 0).is_err());
        assert!(home.is_dir());
        forget_engine(&home, "e1").unwrap();
        assert_eq!(get(&home, b.id).engine, None);
        // A number stays used while its entry is gone but its folder is not reused.
        let c = create(&home, "C", None, None).unwrap();
        assert_eq!(c.id, b.id + 1);
        fs::remove_dir_all(home).unwrap();
    }
}
