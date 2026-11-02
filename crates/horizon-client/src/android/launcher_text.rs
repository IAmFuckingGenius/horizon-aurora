//! The launcher's own words in English and Russian. The English text is the key:
//! `t("Play")` is "Play", or "Играть" when Android's language is Russian.
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Lang {
    En,
    Ru,
}

/// 0 = not detected yet, 1 = English, 2 = Russian.
static LANG: AtomicU8 = AtomicU8::new(0);

/// The launcher's language: Russian when Android's is Russian, else English.
pub(crate) fn lang() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::En,
        2 => Lang::Ru,
        _ => {
            let lang = if crate::system_language().is_some_and(|l| l.eq_ignore_ascii_case("ru")) {
                Lang::Ru
            } else {
                Lang::En
            };
            set(lang);
            lang
        }
    }
}
pub(crate) fn set(lang: Lang) {
    LANG.store(if lang == Lang::Ru { 2 } else { 1 }, Ordering::Relaxed);
}

/// `english` in the launcher's language.
pub(crate) fn t(english: &'static str) -> &'static str {
    if lang() == Lang::En {
        return english;
    }
    RU.iter()
        .find(|(en, _)| *en == english)
        .map_or(english, |(_, ru)| ru)
}

/// A run-time English string in the launcher's language: an exact entry, or one
/// whose first number is the `{}` of an entry ("64 MiB" → "{} MiB").
pub(crate) fn t_dyn(english: &str) -> String {
    if lang() == Lang::En {
        return english.to_owned();
    }
    if let Some((_, ru)) = RU.iter().find(|(en, _)| *en == english) {
        return (*ru).to_owned();
    }
    let Some(start) = english.find(|c: char| c.is_ascii_digit()) else {
        return english.to_owned();
    };
    let end = english[start..]
        .find(|c: char| !c.is_ascii_digit())
        .map_or(english.len(), |e| start + e);
    let template = format!("{}{{}}{}", &english[..start], &english[end..]);
    RU.iter().find(|(en, _)| *en == template).map_or_else(
        || english.to_owned(),
        |(_, ru)| ru.replacen("{}", &english[start..end], 1),
    )
}

/// `english` with `{}` replaced, in order, by `args`.
pub(crate) fn tf(english: &'static str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut args = args.iter();
    let mut rest = t(english);
    while let Some(at) = rest.find("{}") {
        out.push_str(&rest[..at]);
        out.push_str(args.next().copied().unwrap_or_default());
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

static RU: &[(&str, &str)] = &[
    ("Hardcore", "Хардкор"),
    ("Creative", "Творческий"),
    ("Adventure", "Приключение"),
    ("Spectator", "Наблюдатель"),
    ("Survival", "Выживание"),
    ("today", "сегодня"),
    ("yesterday", "вчера"),
    ("{} days ago", "{} дн. назад"),
    ("GiB", "ГиБ"),
    ("MiB", "МиБ"),
    ("K", " тыс."),
    ("M", " млн"),
    ("Something went wrong", "Что-то пошло не так"),
    ("downloading", "загрузка"),
    ("download failed", "ошибка загрузки"),
    ("ready", "готово"),
    ("not installed", "не установлено"),
    ("Player", "Игрок"),
    ("Microsoft", "Microsoft"),
    ("Signing in…", "Вход…"),
    ("Offline", "Офлайн"),
    ("Sign in", "Войти"),
    (
        "Profile «{}». Last world: «{}», {}.",
        "Профиль «{}». Последний мир — «{}», {}.",
    ),
    (
        "Profile «{}». No worlds yet: create one from Play.",
        "Профиль «{}». Миров пока нет — создайте первый через «Играть».",
    ),
    ("Play", "Играть"),
    ("Profile", "Профиль"),
    ("Last world", "Последний мир"),
    ("Continue", "Продолжить"),
    (
        "Your worlds appear here. Press Play, then Singleplayer, to create one.",
        "Здесь появятся ваши миры. Нажмите «Играть» → «Одиночная игра», чтобы создать первый.",
    ),
    ("Quick join a server", "Быстрый вход на сервер"),
    ("Join", "Зайти"),
    ("Server address", "Адрес сервера"),
    ("Main", "Основной"),
    ("Profile {}", "Профиль {}"),
    (
        "Getting ready for the first start",
        "Подготовка к первому запуску",
    ),
    (
        "Minecraft 26.3 is not installed yet",
        "Minecraft 26.3 ещё не установлен",
    ),
    ("Minecraft {} · profile «{}»", "Minecraft {} · профиль «{}»"),
    (
        "Download {}, about 1.1 GiB on disk",
        "Загрузка {}, на диске займёт около 1,1 ГиБ",
    ),
    ("{} free: enough space", "Свободно {} — места хватит"),
    (
        "Only {} free: free up about 1.3 GiB first",
        "Свободно только {} — освободите около 1,3 ГиБ",
    ),
    ("{}/s · about {} min left", "{}/с · осталось около {} мин"),
    ("Measuring the speed…", "Измеряем скорость…"),
    (
        "Keep Horizon open; downloaded files are kept for a retry.",
        "Не закрывайте Horizon; скачанное сохранится для повтора.",
    ),
    ("Cancel", "Отмена"),
    (
        "Horizon downloads the game files from Mojang's servers and checks every file. You need about 1.3 GiB free.",
        "Horizon скачает файлы игры с серверов Mojang и проверит каждый. Понадобится около 1,3 ГиБ свободного места.",
    ),
    ("Install", "Установить"),
    ("Download cancelled", "Загрузка отменена"),
    (
        "Couldn't download the resources",
        "Не получилось скачать ресурсы",
    ),
    (
        "Downloaded files are kept: Retry continues from where it stopped.",
        "Скачанное сохранено — «Повторить» продолжит с этого места.",
    ),
    (
        "Downloaded files are kept: Retry continues from where it stopped. Check the connection, VPN or Private DNS.",
        "Скачанное сохранено — «Повторить» продолжит с этого места. Проверьте интернет, VPN или частный DNS.",
    ),
    (
        "If it keeps failing, clear the download cache: everything downloads again.",
        "Если ошибка повторяется, очистите кеш загрузок: всё скачается заново.",
    ),
    ("Retry", "Повторить"),
    ("Clear download cache", "Очистить кеш загрузок"),
    ("Add-ons", "Дополнения"),
    ("Mods", "Моды"),
    ("Resource packs", "Ресурспаки"),
    ("Shaders", "Шейдеры"),
    ("Browse", "Обзор"),
    ("Installed", "Установлено"),
    ("Import file", "Импорт файла"),
    ("Search resource packs", "Поиск ресурспаков"),
    ("Search shaders", "Поиск шейдеров"),
    (
        "Search mods or paste an .hmod link",
        "Поиск модов или ссылка на .hmod",
    ),
    ("Page {}", "Стр. {}"),
    (
        "Java mods: browse only, they cannot run here",
        "Java-моды: только просмотр, здесь они не запускаются",
    ),
    ("Sandboxed .hmod mods", "Моды .hmod в песочнице"),
    ("Shader ZIPs: experimental", "ZIP-шейдеры: экспериментально"),
    ("Horizon .hshader packs", "Пакеты .hshader для Horizon"),
    ("Disable", "Выключить"),
    ("Enable", "Включить"),
    ("downloads", "загрузок"),
    ("Java mod", "Java-мод"),
    (".hnmod: later", ".hnmod — позже"),
    ("64-bit only", "Только 64-бит"),
    ("Remove", "Удалить"),
    ("Enabled", "Включён"),
    ("Disabled", "Выключен"),
    ("Searching…", "Ищем…"),
    (
        "Results appear here in a moment.",
        "Результаты появятся через мгновение.",
    ),
    ("Nothing installed yet", "Пока ничего не установлено"),
    (
        "Install add-ons from Browse, or import a file from the phone.",
        "Установите дополнения из «Обзора» или импортируйте файл с телефона.",
    ),
    ("Nothing found", "Ничего не нашлось"),
    (
        "Try another search or import a file from the phone.",
        "Попробуйте другой запрос или импортируйте файл с телефона.",
    ),
    ("Clear search", "Сбросить поиск"),
    ("Worlds & files", "Миры и файлы"),
    ("1 world", "1 мир"),
    ("{} worlds", "Миров: {}"),
    ("{} free", "свободно {}"),
    ("Open in Files", "Открыть в Файлах"),
    ("No worlds yet", "Миров пока нет"),
    (
        "Press Play, then Singleplayer, to create one. Copied world folders appear here too.",
        "Нажмите «Играть» → «Одиночная игра», чтобы создать мир. Скопированные папки миров тоже появятся здесь.",
    ),
    ("Settings", "Настройки"),
    (
        "Recommended: fastest on most phones",
        "Рекомендуется: быстрее всего на большинстве телефонов",
    ),
    ("For old drivers; slower", "Для старых драйверов, медленнее"),
    (
        "If plain Vulkan shows glitches or crashes",
        "Если на обычном Vulkan артефакты или вылеты",
    ),
    ("{} · Microsoft", "{} · Microsoft"),
    (
        "Servers that check the licence work",
        "Работают серверы с проверкой лицензии",
    ),
    ("{} · offline", "{} · офлайн"),
    (
        "Singleplayer and servers without licence checks",
        "Одиночная игра и серверы без проверки лицензии",
    ),
    ("Renderer", "Рендер"),
    ("Resolution", "Разрешение"),
    ("Aspect ratio", "Соотношение сторон"),
    ("Refresh rate", "Частота кадров"),
    ("Thread limit", "Лимит потоков"),
    ("Workers that build chunks", "Потоки, которые строят чанки"),
    ("Auto", "Авто"),
    ("RAM cache", "Кеш в памяти"),
    (
        "Buffers the game reuses",
        "Буферы, которые игра переиспользует",
    ),
    ("Default (80 MiB)", "По умолчанию (80 МиБ)"),
    ("Performance hints", "Подсказки производительности"),
    (
        "Android ADPF: steadier frame times",
        "Android ADPF: ровнее время кадра",
    ),
    ("Download cache", "Кеш загрузок"),
    (
        "Installer files kept for a retry",
        "Файлы установщика для повторной попытки",
    ),
    ("Clear", "Очистить"),
    ("Controls", "Управление"),
    (
        "Auto: touch until a keyboard or mouse connects",
        "Авто: сенсор, пока не подключены клавиатура или мышь",
    ),
    ("Touch", "Сенсор"),
    ("Keyboard & mouse", "Клавиатура и мышь"),
    ("Orientation", "Ориентация"),
    ("Full screen", "Полный экран"),
    (
        "Hide the status and navigation bars",
        "Скрыть строку состояния и панель навигации",
    ),
    ("Avoid the camera cutout", "Обходить вырез камеры"),
    (
        "Keep the picture clear of the camera",
        "Камера не закроет угол картинки",
    ),
    ("Account", "Аккаунт"),
    ("Separate worlds and settings", "Отдельные миры и настройки"),
    ("Launch log", "Журнал запуска"),
    ("Errors from the last starts", "Ошибки последних запусков"),
    ("Open", "Открыть"),
    ("Report for developers", "Отчёт для разработчиков"),
    (
        "One ZIP to attach to a bug report",
        "Один ZIP для приложения к баг-репорту",
    ),
    ("Export", "Экспорт"),
    ("Record performance", "Запись производительности"),
    (
        "Up to 30 min, for slow-game reports",
        "До 30 мин, для отчётов о тормозах",
    ),
    ("GPU profiling", "Профилирование GPU"),
    (
        "Adds GPU timings to the recording",
        "Добавляет время GPU в запись",
    ),
    ("Raw performance data", "Сырые данные производительности"),
    ("JSONL for analysis", "JSONL для анализа"),
    ("Game files", "Файлы игры"),
    (
        "Worlds, mods, packs and reports in Files",
        "Миры, моды, паки и отчёты в «Файлах»",
    ),
    ("Version", "Версия"),
    ("Source code", "Исходный код"),
    (
        "Your worlds are saved. Downloads, if any, stop and resume next time.",
        "Миры сохранены. Загрузки, если идут, остановятся и продолжатся при следующем запуске.",
    ),
    ("Leave Horizon?", "Выйти из Horizon?"),
    ("Leave", "Выйти"),
    ("Stay", "Остаться"),
    ("Sign in with Microsoft", "Вход через Microsoft"),
    (
        "Signed in as {}. The session renews by itself.",
        "Вы вошли как {}. Сеанс продлевается сам.",
    ),
    ("Switch account", "Сменить аккаунт"),
    ("Sign out", "Выйти из аккаунта"),
    (
        "Needed for servers that check the licence. On another device open microsoft.com/link and enter the code:",
        "Нужен для серверов с проверкой лицензии. На другом устройстве откройте microsoft.com/link и введите код:",
    ),
    ("Copy", "Копировать"),
    ("Open the link", "Открыть ссылку"),
    ("Waiting for confirmation…", "Ждём подтверждения…"),
    ("Requesting a sign-in code…", "Запрашиваем код входа…"),
    (
        "Needed for servers that check the licence. You type your password on Microsoft's own page.",
        "Нужен для серверов с проверкой лицензии. Пароль вводится на странице Microsoft.",
    ),
    ("Sign in with the browser", "Войти через браузер"),
    (
        "Use a code on another device",
        "Ввести код на другом устройстве",
    ),
    ("Cancel sign-in", "Отменить вход"),
    (
        "Request Minecraft API approval",
        "Запросить одобрение Minecraft API",
    ),
    ("Without signing in", "Без входа"),
    (
        "Singleplayer worlds and servers without licence checks.",
        "Одиночные миры и серверы без проверки лицензии.",
    ),
    ("In-game name", "Имя в игре"),
    ("Name", "Имя"),
    ("Save name", "Сохранить имя"),
    ("Connecting to {}…", "Подключение к {}…"),
    (
        "Wait for the download to finish",
        "Дождитесь окончания загрузки",
    ),
    ("Download cache cleared", "Кеш загрузок очищен"),
    ("Could not clear the cache", "Не удалось очистить кеш"),
    ("Downloaded files kept", "Скачанное сохранено"),
    ("Home", "Главная"),
    ("Worlds", "Миры"),
    ("Checking the version", "Проверка версии"),
    (
        "Downloading resources and sounds",
        "Загрузка ресурсов и звуков",
    ),
    ("Unpacking", "Распаковка"),
    ("Preparing worlds", "Подготовка миров"),
    ("Touch controls", "Сенсорные кнопки"),
    ("Finishing", "Завершение"),
    ("Graphics", "Графика"),
    ("Performance", "Производительность"),
    ("Input", "Ввод"),
    ("Display", "Экран"),
    ("Diagnostics & about", "Диагностика и о программе"),
    ("Reusable RAM cache", "Кеш в памяти"),
    ("Chunk worker limit", "Лимит потоков чанков"),
    ("Display refresh rate", "Частота обновления экрана"),
    (
        "An Android display preference, not a game FPS cap. Unsupported rates may be ignored.",
        "Настройка экрана Android, а не ограничение FPS игры. Неподдерживаемые частоты могут игнорироваться.",
    ),
    ("Client resolution", "Разрешение"),
    (
        "Small screens retain a usable menu size.",
        "На маленьких экранах меню остаётся удобного размера.",
    ),
    ("Screen orientation", "Ориентация экрана"),
    (
        "Applies to the launcher and game. Does not change touch bindings.",
        "Для лаунчера и игры. Сенсорные кнопки не меняются.",
    ),
    ("Rendering mode", "Режим рендера"),
    (
        "Uses the system GPU driver. Applies on the next game launch.",
        "Использует системный драйвер GPU. Применится при следующем запуске игры.",
    ),
    ("Choose profile", "Выбор профиля"),
    (
        "Each profile keeps its own game settings and saves.",
        "У каждого профиля свои настройки и сохранения.",
    ),
    ("Remove add-on?", "Удалить дополнение?"),
    (
        "Deletes only the selected file from this profile. This cannot be undone.",
        "Удалит только выбранный файл из этого профиля. Отменить нельзя.",
    ),
    ("OK", "ОК"),
    (
        "Sounds and mesh buffers the game keeps for reuse. Not a limit on all memory. Applies on the next Play.",
        "Звуки и буферы мешей, которые игра хранит для повторного использования. Это не лимит всей памяти. Применится при следующем запуске.",
    ),
    (
        "A ceiling for the threads that generate and build chunks. Fewer ease the CPU but load chunks slower. Applies on the next Play.",
        "Потолок для потоков, которые генерируют и строят чанки. Меньше — легче процессору, но чанки грузятся медленнее. Применится при следующем запуске.",
    ),
    (
        "Fits the whole picture without stretching. In portrait, width and height swap.",
        "Вписывает картинку целиком без растяжения. В портретной ориентации стороны меняются местами.",
    ),
    ("Vulkan / GPU batching", "Vulkan / пакетная отрисовка"),
    ("OpenGL ES / experimental", "OpenGL ES / экспериментально"),
    ("Vulkan / compatibility", "Vulkan / совместимость"),
    (
        "For older GPUs. Requires ES 3.0+ and sufficient driver limits.",
        "Для старых GPU. Нужен ES 3.0+ и достаточные лимиты драйвера.",
    ),
    (
        "Groups terrain draws and enables supported GPU culling.",
        "Группирует отрисовку ландшафта и включает отсечение на GPU.",
    ),
    (
        "Direct terrain draws for drivers with missing or transparent blocks.",
        "Прямая отрисовка ландшафта для драйверов, где пропадают или просвечивают блоки.",
    ),
    ("Landscape (sensor)", "Альбомная (датчик)"),
    ("Portrait (sensor)", "Портретная (датчик)"),
    ("Landscape (reversed)", "Альбомная (перевёрнутая)"),
    ("Portrait (reversed)", "Портретная (перевёрнутая)"),
    ("Automatic rotation", "Автоповорот"),
    ("Native", "Как у экрана"),
    ("Engine default (80 MiB)", "По умолчанию (80 МиБ)"),
    ("{} MiB", "{} МиБ"),
    ("No additional limit", "Без ограничения"),
    ("{} worker(s)", "Потоков: {}"),
    ("System default", "Как в системе"),
    ("{} Hz", "{} Гц"),
    ("{} Hz (request)", "{} Гц (запрос)"),
    ("Native resolution", "Родное разрешение"),
    (
        "Lower resolution for the whole picture",
        "Ниже разрешение всей картинки",
    ),
    ("Custom percentage...", "Свой процент…"),
    (
        "Enter a value from 25 to 100",
        "Введите значение от 25 до 100",
    ),
    ("Default profile", "Профиль по умолчанию"),
    (
        "Separate saves, mods and packs",
        "Отдельные сохранения, моды и паки",
    ),
    ("Create profile", "Создать профиль"),
    ("Create Profile {}", "Создать профиль {}"),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn russian_entries_are_unique_and_keep_their_placeholders() {
        let mut seen = std::collections::HashSet::new();
        for (en, ru) in RU {
            assert!(seen.insert(*en), "duplicate key {en:?}");
            assert_eq!(en.matches("{}").count(), ru.matches("{}").count(), "{en:?}");
        }
    }
    #[test]
    fn placeholders_fill_in_order() {
        set(Lang::En);
        assert_eq!(tf("{} of {}", &["2", "6"]), "2 of 6");
    }
}
