//! Which folders of Horizon Android (or Starling: the same app) a move takes, and the
//! names it accepts from the old app's documents provider (`horizon_import.rs` copies
//! them). No Android here: `tools/test_android_worlds.py` runs these tests on the host.
//!
//! The old provider's tree is `document_store.rs`'s: the root `horizon` holds
//! `p<profile>/<folder>` for its current profile, with path-like ids below them.

/// The old app's documents providers: Horizon Android or Starling, and their dev builds.
pub const AUTHORITIES: [&str; 2] = [
    "dev.horizon.android.documents",
    "dev.horizon.android.dev.documents",
];
/// The old app's root document.
pub const ROOT: &str = "horizon";
/// Its folders and where each goes in an instance's folder ("" the folder itself).
pub const FOLDERS: [(&str, &str); 5] = [
    ("worlds", "saves"),
    ("packs", "resourcepacks"),
    ("shaders", "shaderpacks"),
    ("mods", "horizon-mods"),
    ("configuration", ""),
];

/// Whether `uri` is a folder of the old app's provider (a tree the picker granted).
pub fn from_horizon(uri: &str) -> bool {
    uri.strip_prefix("content://")
        .and_then(|rest| rest.split_once("/tree/"))
        .is_some_and(|(authority, _)| AUTHORITIES.contains(&authority))
}

/// The folders a picked tree holds: the root holds them all, `p<n>/<folder>` one.
/// None when the pick is something else (a world inside, another app's folder).
pub fn folders_of(tree_id: &str, children: &[String]) -> Option<Vec<(String, &'static str)>> {
    let known = |id: &str| {
        let (profile, folder) = id.split_once('/')?;
        let number = profile.strip_prefix('p')?;
        (!number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())).then_some(())?;
        FOLDERS
            .iter()
            .find(|(name, _)| *name == folder)
            .map(|(_, target)| (id.to_owned(), *target))
    };
    if tree_id == ROOT {
        let found: Vec<_> = children.iter().filter_map(|id| known(id)).collect();
        (!found.is_empty()).then_some(found)
    } else {
        known(tree_id).map(|folder| vec![folder])
    }
}

/// A name the old provider listed, safe as one path component here.
pub fn safe_component(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 240
        && !name.chars().any(|c| c.is_control() || "/\\:".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_old_apps_folders_are_taken() {
        assert!(from_horizon(
            "content://dev.horizon.android.documents/tree/horizon"
        ));
        assert!(from_horizon(
            "content://dev.horizon.android.dev.documents/tree/p0%2Fworlds"
        ));
        // Aurora's own provider, a file instead of a folder, another app.
        assert!(!from_horizon(
            "content://io.github.iamfuckinggenius.aurora.documents/tree/horizon"
        ));
        assert!(!from_horizon(
            "content://dev.horizon.android.documents/document/horizon"
        ));
        assert!(!from_horizon(
            "content://com.android.externalstorage.documents/tree/primary%3AHorizon"
        ));
    }

    #[test]
    fn the_root_brings_every_folder_and_a_folder_itself() {
        let children = [
            "p2/mods".to_owned(),
            "p2/packs".to_owned(),
            "p2/shaders".to_owned(),
            "p2/worlds".to_owned(),
            "p2/configuration".to_owned(),
            "reports".to_owned(),
        ];
        let all = folders_of(ROOT, &children).unwrap();
        assert_eq!(all.len(), 5, "reports stay behind");
        assert!(all.contains(&("p2/worlds".to_owned(), "saves")));
        assert!(all.contains(&("p2/configuration".to_owned(), "")));
        assert_eq!(
            folders_of("p0/packs", &[]),
            Some(vec![("p0/packs".to_owned(), "resourcepacks")])
        );
        // A world inside, the reports, something unknown.
        assert_eq!(folders_of("p0/worlds/My World", &[]), None);
        assert_eq!(folders_of("reports", &[]), None);
        assert_eq!(folders_of("px/worlds", &[]), None);
        assert_eq!(folders_of(ROOT, &["reports".to_owned()]), None);
    }

    #[test]
    fn listed_names_cannot_leave_their_folder() {
        assert!(safe_component("New World"));
        assert!(safe_component("r.0.0.mca"));
        for name in ["", ".", "..", "a/b", "a\\b", "c:", "bell\u{7}"] {
            assert!(!safe_component(name), "{name:?}");
        }
    }
}
