//! The official native mods' words (Zoom, Minimap, Recipes and SmartF3: Android's bundle, and
//! the same mods built into a desktop game) in the game's language. The mods label their
//! Options > Mods rows and settings buttons in English; each label they make is read back
//! phrase by phrase and said again through Horizon's bundled strings (`horizon.mod.*`). Text
//! these patterns don't know stays as the mod wrote it.
use horizon_client::{
    lang,
    text::{self, Text},
};

/// A bundled string by key: the current language's, else `None` (the mod's English stands).
fn current(key: &str) -> Option<String> {
    lang::current().and_then(|l| l.get(key).map(str::to_owned))
}

/// Mod `id`'s settings button `english`, in the game's language.
pub(super) fn settings_label(id: &str, english: &str) -> String {
    settings_label_in(&current, id, english)
}
/// The caption of mod `id`'s key `name` on its page.
pub(super) fn key_label(id: &str, name: &str, english: &str) -> String {
    current(&format!("horizon.mod.{id}.key.{name}")).unwrap_or_else(|| english.to_owned())
}
/// A mod's description: a language that has it keys it by the English sentence, so a mod
/// that rewords it reads as it wrote it.
pub(super) fn description(english: &str) -> String {
    current(english).unwrap_or_else(|| english.to_owned())
}

/// `template` with its arguments (`TranslatableContents`' `%s` and `%N$s`).
fn fill(template: &str, args: &[&str]) -> String {
    let args: Vec<Text> = args.iter().map(|a| Text::literal(*a)).collect();
    text::format_translation(template, &args).plain()
}

/// A language's words for the patterns below: a key's text, or the English given with it.
struct Words<'a>(&'a dyn Fn(&str) -> Option<String>);
impl Words<'_> {
    fn get(&self, key: &str, english: &str) -> String {
        (self.0)(key).unwrap_or_else(|| english.to_owned())
    }
    fn fill(&self, key: &str, english: &str, args: &[&str]) -> String {
        fill(&self.get(key, english), args)
    }
    /// The mods' `ON`/`OFF`: vanilla's own words for them.
    fn on_off(&self, value: &str) -> Option<String> {
        match value {
            "ON" => Some(self.get("options.on", "ON")),
            "OFF" => Some(self.get("options.off", "OFF")),
            _ => None,
        }
    }
    /// One of `choices` (English name, key suffix) under `prefix`.
    fn choice(&self, prefix: &str, value: &str, choices: &[(&str, &str)]) -> Option<String> {
        let (english, id) = choices.iter().find(|(english, _)| *english == value)?;
        Some(self.get(&format!("{prefix}.{id}"), english))
    }
}

/// `settings_label` with the words of a given language (tests pass one).
fn settings_label_in(words: &dyn Fn(&str) -> Option<String>, id: &str, english: &str) -> String {
    let w = Words(words);
    let text = english.trim_start();
    let indent = &english[..english.len() - text.len()];
    let said = match id {
        "zoom" => zoom(&w, text),
        "minimap" => minimap(&w, text),
        "recipes" => recipes(&w, text),
        "smartf3" => smartf3(&w, text),
        _ => None,
    };
    said.map_or_else(|| english.to_owned(), |said| format!("{indent}{said}"))
}

fn zoom(w: &Words, text: &str) -> Option<String> {
    if let Some(level) = text
        .strip_prefix("Starting zoom: ")
        .and_then(|v| v.strip_suffix('x'))
    {
        return Some(w.fill("horizon.mod.zoom.level", "Starting zoom: %sx", &[level]));
    }
    let (caption, value) = text.split_once(": ")?;
    let key = match caption {
        "Smooth zoom" => "horizon.mod.zoom.smooth",
        "Slower turning" => "horizon.mod.zoom.sensitivity",
        _ => return None,
    };
    Some(w.fill(key, &format!("{caption}: %s"), &[&w.on_off(value)?]))
}

fn minimap(w: &Words, text: &str) -> Option<String> {
    if let Some(blocks) = text
        .strip_prefix("Zoom: ")
        .and_then(|v| v.strip_suffix(" per pixel"))
    {
        let blocks = match blocks {
            "½ block" => w.get("horizon.mod.minimap.blocks.half", "½ block"),
            "1 block" => w.get("horizon.mod.minimap.blocks.one", "1 block"),
            many => {
                let n = many.strip_suffix(" blocks")?;
                w.fill("horizon.mod.minimap.blocks.many", "%s blocks", &[n])
            }
        };
        return Some(w.fill("horizon.mod.minimap.zoom", "Zoom: %s per pixel", &[&blocks]));
    }
    let (caption, value) = text.split_once(": ")?;
    let (key, value) = match caption {
        "Corner" => (
            "horizon.mod.minimap.corner",
            w.choice(
                "horizon.mod.minimap.corner",
                value,
                &[
                    ("Top left", "top_left"),
                    ("Top right", "top_right"),
                    ("Bottom left", "bottom_left"),
                    ("Bottom right", "bottom_right"),
                ],
            )?,
        ),
        "Size" => (
            "horizon.mod.minimap.size",
            w.choice(
                "horizon.mod.minimap.size",
                value,
                &[("Small", "small"), ("Medium", "medium"), ("Large", "large")],
            )?,
        ),
        "Shape" => (
            "horizon.mod.minimap.shape",
            w.choice(
                "horizon.mod.minimap.shape",
                value,
                &[("Round", "round"), ("Square", "square")],
            )?,
        ),
        "Turn with the player" => ("horizon.mod.minimap.rotate", w.on_off(value)?),
        _ => return None,
    };
    Some(w.fill(key, &format!("{caption}: %s"), &[&value]))
}

fn recipes(w: &Words, text: &str) -> Option<String> {
    let value = text.strip_prefix("Click gives items (creative): ")?;
    Some(w.fill(
        "horizon.mod.recipes.give",
        "Click gives items (creative): %s",
        &[&w.on_off(value)?],
    ))
}

/// SmartF3's modules as its page names them, and their keys.
const SMARTF3_MODULES: [(&str, &str); 14] = [
    ("FPS", "fps"),
    ("Server", "server"),
    ("Coordinates", "coordinates"),
    ("Chunk", "chunk"),
    ("Facing", "facing"),
    ("World", "world"),
    ("Biome", "biome"),
    ("Light", "light"),
    ("Chart keys", "charts"),
    ("System (memory, CPU, GPU)", "system"),
    ("Rendering", "rendering"),
    ("Targeted block", "target_block"),
    ("Targeted fluid", "target_fluid"),
    ("Targeted entity", "target_entity"),
];
/// SmartF3's palette as its page names it, and the colours' keys.
const SMARTF3_COLOURS: [(&str, &str); 17] = [
    ("White", "white"),
    ("F3 Grey", "f3_gray"),
    ("Grey", "gray"),
    ("Dark Grey", "dark_gray"),
    ("Black", "black"),
    ("Red", "red"),
    ("Gold", "gold"),
    ("Yellow", "yellow"),
    ("Green", "green"),
    ("Aqua", "aqua"),
    ("Blue", "blue"),
    ("Light Purple", "light_purple"),
    ("Dark Red", "dark_red"),
    ("Dark Green", "dark_green"),
    ("Dark Aqua", "dark_aqua"),
    ("Dark Blue", "dark_blue"),
    ("Dark Purple", "dark_purple"),
];

fn smartf3(w: &Words, text: &str) -> Option<String> {
    const P: &str = "horizon.mod.smartf3";
    let module = |title: &str| w.choice(&format!("{P}.module"), title, &SMARTF3_MODULES);
    let colour = |name: &str| {
        if name.starts_with('#') {
            Some(name.to_owned())
        } else {
            w.choice(&format!("{P}.colour"), name, &SMARTF3_COLOURS)
        }
    };
    if text == "Reset to defaults" {
        return Some(w.get(&format!("{P}.reset"), text));
    }
    if let Some(problem) = text.strip_suffix(" (click to read it again)") {
        return Some(w.fill(
            &format!("{P}.reload"),
            "%s (click to read it again)",
            &[problem],
        ));
    }
    if let Some(rest) = text
        .strip_prefix("Module ")
        .and_then(|r| r.strip_suffix(" (click for the next)"))
    {
        let (at, title) = rest.split_once(": ")?;
        return Some(w.fill(
            &format!("{P}.module_pick"),
            "Module %s: %s (click for the next)",
            &[at, &module(title)?],
        ));
    }
    for (verb, key) in [("Move up", "move_up"), ("Move down", "move_down")] {
        // "Move up (2nd of 5 in the left column)", "Move up (already first in the left column)".
        let Some(inner) = text
            .strip_prefix(verb)
            .and_then(|r| r.strip_prefix(" ("))
            .and_then(|r| r.strip_suffix(" column)"))
        else {
            continue;
        };
        let (place, column) = inner.rsplit_once(" in the ")?;
        let column = w.choice(
            &format!("{P}.in_column"),
            column,
            &[("left", "left"), ("right", "right")],
        )?;
        if place.starts_with("already ") {
            let english = format!("{verb} ({place} in the %s column)");
            return Some(w.fill(&format!("{P}.{key}_end"), &english, &[&column]));
        }
        let (ordinal, count) = place.split_once(" of ")?;
        let number = ordinal.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        let english = format!("{verb} (%1$s of %3$s in the %4$s column)");
        return Some(w.fill(
            &format!("{P}.{key}"),
            &english,
            &[ordinal, number, count, &column],
        ));
    }
    let (caption, value) = text.split_once(": ")?;
    if let Some(title) = module(caption) {
        let state = w.choice(
            &format!("{P}.state"),
            value,
            &[("shown", "shown"), ("hidden", "hidden")],
        )?;
        return Some(format!("{title}: {state}"));
    }
    let (key, value) = match caption {
        "Debug screen" => (
            "screen",
            match value {
                "vanilla F3" => w.get(&format!("{P}.screen.vanilla"), value),
                other => other.to_owned(),
            },
        ),
        "Layout" => (
            "layout",
            w.choice(
                &format!("{P}.layout"),
                value,
                &[("compact", "compact"), ("full", "full")],
            )?,
        ),
        "Text shadow" => ("shadow", w.on_off(value)?),
        "Background" => (
            "background",
            w.choice(
                &format!("{P}.background"),
                value,
                &[
                    ("grey (vanilla)", "vanilla"),
                    ("dark", "dark"),
                    ("none", "none"),
                ],
            )?,
        ),
        "Gap between modules" => ("spacing", w.on_off(value)?),
        "Label colour" => ("label_colour", colour(value)?),
        "Value colour" => ("value_colour", colour(value)?),
        "Column" => (
            "column",
            w.choice(
                &format!("{P}.column"),
                value,
                &[("left", "left"), ("right", "right")],
            )?,
        ),
        _ => return None,
    };
    Some(w.fill(&format!("{P}.{key}"), &format!("{caption}: %s"), &[&value]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_client::lang::Language;

    /// The labels the official mods make (their runtimes' own tests show these).
    const LABELS: [(&str, &str); 16] = [
        ("zoom", "Starting zoom: 3x"),
        ("zoom", "Smooth zoom: OFF"),
        ("zoom", "Slower turning: ON"),
        ("minimap", "Corner: Bottom left"),
        ("minimap", "Size: Large"),
        ("minimap", "Shape: Round"),
        ("minimap", "Zoom: ½ block per pixel"),
        ("minimap", "Zoom: 4 blocks per pixel"),
        ("minimap", "Turn with the player: ON"),
        ("recipes", "Click gives items (creative): ON"),
        ("smartf3", "Debug screen: vanilla F3"),
        ("smartf3", "Module 3/14: Chunk (click for the next)"),
        ("smartf3", "  Chunk: hidden"),
        ("smartf3", "  Label colour: Dark Aqua"),
        ("smartf3", "  Move up (2nd of 5 in the left column)"),
        ("smartf3", "  Move down (already last in the right column)"),
    ];

    #[test]
    fn the_official_mods_labels_read_as_they_wrote_them_in_english() {
        let en = Language::load(&[], "en_us");
        let words = |key: &str| en.get(key).map(str::to_owned);
        for (id, label) in LABELS {
            assert_eq!(settings_label_in(&words, id, label), label);
        }
        // Another mod's label, or one the patterns don't know, is left alone.
        assert_eq!(
            settings_label_in(&words, "veinminer", "Size: Large"),
            "Size: Large"
        );
        assert_eq!(
            settings_label_in(&words, "zoom", "Size: Large"),
            "Size: Large"
        );
    }

    #[test]
    fn the_official_mods_labels_read_in_russian() {
        let ru = Language::load(&[], "ru_ru");
        // Vanilla's ON/OFF come from the prepared resources, which tests lack.
        let words = |key: &str| match key {
            "options.on" => Some("Вкл".to_owned()),
            "options.off" => Some("Выкл".to_owned()),
            _ => ru.get(key).map(str::to_owned),
        };
        let russian: Vec<String> = LABELS
            .iter()
            .map(|(id, label)| settings_label_in(&words, id, label))
            .collect();
        assert_eq!(
            russian,
            [
                "Начальный зум: 3x",
                "Плавный зум: Выкл",
                "Медленный поворот: Вкл",
                "Угол: Снизу слева",
                "Размер: Большой",
                "Форма: Круглая",
                "Масштаб: ½ блока на пиксель",
                "Масштаб: 4 блока на пиксель",
                "Поворот вместе с игроком: Вкл",
                "Выдача предметов щелчком (творчество): Вкл",
                "Экран отладки: Ванильный F3",
                "Модуль 3/14: Чанк (далее по щелчку)",
                "  Чанк: Выкл",
                "  Цвет подписи: Бирюзовый",
                "  Выше (2-й из 5 в левом столбце)",
                "  Ниже (уже последний в правом столбце)",
            ]
        );
    }
}
