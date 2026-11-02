//! Saved touch layouts: the player's named layouts in `horizon-touch-layouts/`, and a
//! short text code that carries one layout through the clipboard (a chat, a note).
//! A layout is where everything is and what shows: the three schemes' placements,
//! the controls' visibility, the custom and utility buttons and the global button
//! size. Sensitivity, the scheme and other preferences stay the player's own.
use super::layout::{Placement, Settings};
use super::{CustomButton, UtilityLayout};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The folder of saved layouts, beside `horizon-touch.json` in the game folder.
pub const DIR: &str = "horizon-touch-layouts";
/// What a layout code starts with; the version follows.
const CODE_PREFIX: &str = "HZL1.";
/// The longest name kept, in characters.
pub const NAME_LIMIT: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutProfile {
    pub name: String,
    #[serde(deserialize_with = "super::layout::deserialize_layouts")]
    pub layouts: [[Placement; 14]; 3],
    #[serde(deserialize_with = "super::layout::deserialize_visibility")]
    pub button_visibility: [u8; 14],
    #[serde(default)]
    pub custom_buttons: Vec<CustomButton>,
    #[serde(default)]
    pub utility: UtilityLayout,
    #[serde(default = "one")]
    pub button_scale: f32,
}
fn one() -> f32 {
    1.0
}
impl LayoutProfile {
    /// The layout part of `settings`, under `name`.
    pub fn capture(settings: &Settings, name: &str) -> Self {
        Self {
            name: clean_name(name),
            layouts: settings.layouts,
            button_visibility: settings.button_visibility,
            custom_buttons: settings.custom_buttons.clone(),
            utility: settings.utility,
            button_scale: settings.button_scale,
        }
    }
    /// Puts this layout into `settings`, keeping every other preference.
    pub fn apply(&self, settings: &mut Settings) {
        settings.layouts = self.layouts;
        settings.button_visibility = self.button_visibility;
        settings.custom_buttons = self.custom_buttons.clone();
        settings.utility = self.utility;
        settings.button_scale = self.button_scale;
        settings.sanitize();
    }
    /// A one-line code for the clipboard: compressed JSON in URL-safe base64.
    pub fn to_code(&self) -> String {
        use base64::Engine;
        use std::io::Write;
        let json = serde_json::to_vec(self).unwrap_or_default();
        let mut deflate =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        let packed = deflate
            .write_all(&json)
            .and_then(|()| deflate.finish())
            .unwrap_or_default();
        // The closing dot ends the code, so words a chat puts after it don't join it.
        format!(
            "{CODE_PREFIX}{}.",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(packed)
        )
    }
    /// Reads a code made by [`Self::to_code`]; spaces and line breaks a chat added
    /// around or inside it are ignored. Errors are short English reasons for logs.
    pub fn from_code(text: &str) -> Result<Self, &'static str> {
        use base64::Engine;
        use std::io::Read;
        let start = text.find(CODE_PREFIX).ok_or("not a layout code")?;
        // Up to the closing dot; line breaks a chat put inside the code are dropped.
        let body: String = text[start + CODE_PREFIX.len()..]
            .chars()
            .take_while(|c| *c != '.')
            .filter(|c| !c.is_whitespace())
            .collect();
        let packed = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| "damaged layout code")?;
        let mut json = Vec::new();
        flate2::read::DeflateDecoder::new(packed.as_slice())
            .take(4 << 20)
            .read_to_end(&mut json)
            .map_err(|_| "damaged layout code")?;
        let mut profile: Self = serde_json::from_slice(&json).map_err(|_| "unreadable layout")?;
        profile.name = clean_name(&profile.name);
        profile.custom_buttons.truncate(32);
        profile
            .custom_buttons
            .retain(|b| CustomButton::valid_token(&b.key));
        Ok(profile)
    }
}

/// A name as the player sees it: trimmed, single-line, at most [`NAME_LIMIT`].
pub fn clean_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(NAME_LIMIT)
        .collect()
}
/// The file a name is saved under: letters and digits kept (any script), the rest
/// turned into `_`, so two names that differ only in punctuation share a file.
fn file_name(name: &str) -> String {
    let stem: String = clean_name(name)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let stem = stem.trim_matches('_');
    format!("{}.json", if stem.is_empty() { "layout" } else { stem })
}

/// The saved layouts in `dir`, by name.
pub fn list(dir: &Path) -> Vec<LayoutProfile> {
    let mut out: Vec<LayoutProfile> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|b| serde_json::from_slice::<LayoutProfile>(&b).ok())
        .filter(|p| !p.name.is_empty())
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}
/// Saves `profile` in `dir`, replacing a layout of the same name.
pub fn save(dir: &Path, profile: &LayoutProfile) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name(&profile.name));
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_vec_pretty(profile)?)?;
    std::fs::rename(&temp, &path)?;
    Ok(path)
}
/// Deletes the layout named `name` from `dir`.
pub fn delete(dir: &Path, name: &str) -> std::io::Result<()> {
    std::fs::remove_file(dir.join(file_name(name)))
}
/// A name not taken in `dir`: `base`, or `base 2`, `base 3`...
pub fn free_name(dir: &Path, base: &str) -> String {
    let taken: Vec<String> = list(dir).into_iter().map(|p| file_name(&p.name)).collect();
    let base = clean_name(base);
    if !taken.contains(&file_name(&base)) {
        return base;
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|name| !taken.contains(&file_name(name)))
        .unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "horizon-touch-layouts-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }
    #[test]
    fn a_layout_survives_its_code_with_chat_whitespace_and_keeps_other_preferences() {
        let mut settings = Settings::default();
        settings.layouts[1][3].center = [0.31, 0.62];
        settings.button_visibility[5] = 3;
        settings.button_scale = 1.4;
        settings
            .custom_buttons
            .push(CustomButton::new("key.keyboard.q".into(), 0));
        let code = LayoutProfile::capture(&settings, "  Мой\nPvP  ").to_code();
        assert!(code.starts_with(CODE_PREFIX) && !code.contains(char::is_whitespace));
        let pasted = format!("look: {}\n{} thanks", &code[..20], &code[20..]);
        let profile = LayoutProfile::from_code(&pasted).unwrap();
        assert_eq!(profile.name, "Мой PvP");
        let mut other = Settings {
            sensitivity: 2.0,
            left_handed: true,
            ..Settings::default()
        };
        profile.apply(&mut other);
        assert_eq!(other.layouts[1][3].center, [0.31, 0.62]);
        assert_eq!(other.button_visibility[5], 3);
        assert_eq!(other.button_scale, 1.4);
        assert_eq!(other.custom_buttons[0].key, "key.keyboard.q");
        assert_eq!(
            other.sensitivity, 2.0,
            "a layout is not the player's sensitivity"
        );
        assert!(other.left_handed);
        assert!(LayoutProfile::from_code("hello").is_err());
        assert!(LayoutProfile::from_code("HZL1.AAAA").is_err());
    }
    #[test]
    fn saved_layouts_list_by_name_replace_their_namesake_and_delete() {
        let dir = temp_dir("files");
        let settings = Settings::default();
        save(&dir, &LayoutProfile::capture(&settings, "Строитель")).unwrap();
        save(&dir, &LayoutProfile::capture(&settings, "aim")).unwrap();
        let mut moved = settings.clone();
        moved.layouts[0][0].center = [0.2, 0.7];
        save(&dir, &LayoutProfile::capture(&moved, "aim")).unwrap();
        let names: Vec<String> = list(&dir).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["aim", "Строитель"]);
        assert_eq!(list(&dir)[0].layouts[0][0].center, [0.2, 0.7]);
        assert_eq!(free_name(&dir, "aim"), "aim 2");
        assert_eq!(free_name(&dir, "new"), "new");
        delete(&dir, "aim").unwrap();
        assert_eq!(list(&dir).len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
