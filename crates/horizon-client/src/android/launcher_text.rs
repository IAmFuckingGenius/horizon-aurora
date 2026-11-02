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
    // Numbers become `{}` ("12 results" → "{} results"), filled back in order.
    let mut template = String::new();
    let mut numbers = Vec::new();
    let mut digits = String::new();
    for ch in english.chars().chain(std::iter::once('\0')) {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }
        if !digits.is_empty() {
            template.push_str("{}");
            numbers.push(std::mem::take(&mut digits));
        }
        if ch != '\0' {
            template.push(ch);
        }
    }
    if numbers.is_empty() {
        return english.to_owned();
    }
    let Some((_, ru)) = RU.iter().find(|(en, _)| *en == template) else {
        return english.to_owned();
    };
    let refs: Vec<&str> = numbers.iter().map(String::as_str).collect();
    let mut out = String::new();
    let mut rest = *ru;
    let mut args = refs.iter();
    while let Some(at) = rest.find("{}") {
        out.push_str(&rest[..at]);
        out.push_str(args.next().copied().unwrap_or_default());
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
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
    (
        "The shape of the game picture. A shape other than the screen's leaves black bars at the edges.",
        "Форма картинки игры. Если она отличается от формы экрана, по краям будут чёрные полосы.",
    ),
    (
        "How often the screen redraws. Higher is smoother but uses more battery.",
        "Как часто обновляется экран. Выше — плавнее, но больше расход батареи.",
    ),
    (
        "Lower is faster but blurrier.",
        "Ниже — быстрее, но картинка мыльнее.",
    ),
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
    (
        "For phones with old Vulkan drivers: slower, but works almost everywhere.",
        "Для телефонов со старыми драйверами Vulkan: медленнее, но работает почти везде.",
    ),
    (
        "Vulkan with driver workarounds: pick it if regular Vulkan shows glitches or crashes.",
        "Vulkan с обходами ошибок драйверов: выберите, если на обычном Vulkan есть артефакты или вылеты.",
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
    (
        "How many processor threads load and build the world around you. Fewer heat the phone less, but the world loads slower.",
        "Сколько потоков процессора загружают и строят мир вокруг. Меньше — телефон меньше греется, но мир прогружается медленнее.",
    ),
    ("Auto", "Авто"),
    ("RAM cache", "Кеш в памяти"),
    (
        "Memory where the game keeps sounds and chunk geometry to reuse. More means fewer stutters on phones with plenty of RAM.",
        "Память, где игра держит звуки и геометрию чанков для повторного использования. Больше — меньше подтормаживаний, если у телефона много ОЗУ.",
    ),
    ("Default (80 MiB)", "По умолчанию (80 МиБ)"),
    ("Performance hints", "Подсказки производительности"),
    (
        "Tells Android how much processor power the game needs (ADPF), so frames come at a steadier pace.",
        "Сообщает Android, сколько мощности процессора нужно игре (ADPF), чтобы кадры шли ровнее.",
    ),
    ("Download cache", "Кеш загрузок"),
    (
        "Minecraft files left after installing, so an interrupted install can continue. Clearing frees space.",
        "Файлы Minecraft, оставшиеся после установки, чтобы прерванная установка могла продолжиться. Очистка освободит место.",
    ),
    ("Clear", "Очистить"),
    ("Controls", "Управление"),
    (
        "Auto uses touch controls and switches to a keyboard or mouse as soon as one is connected.",
        "Авто — сенсорное управление, а когда подключаете клавиатуру или мышь, игра переключается на них.",
    ),
    ("Touch", "Сенсор"),
    ("Keyboard & mouse", "Клавиатура и мышь"),
    (
        "Hides Android's status bar and navigation buttons while you play.",
        "Скрывать строку состояния и кнопки навигации Android во время игры.",
    ),
    ("Avoid the camera cutout", "Обходить вырез камеры"),
    (
        "Keeps the game picture out of the camera cutout, so nothing hides behind it.",
        "Не рисовать игру под вырезом камеры, чтобы ничего не пряталось за ним.",
    ),
    ("Account", "Аккаунт"),
    (
        "Each profile keeps its own worlds and settings.",
        "У каждого профиля свои миры и настройки.",
    ),
    ("Launch log", "Журнал запуска"),
    (
        "Shows the errors from the last game launches.",
        "Показывает ошибки последних запусков игры.",
    ),
    ("Open", "Открыть"),
    ("Report for developers", "Отчёт для разработчиков"),
    (
        "Saves logs and device details into one ZIP to attach to a bug report.",
        "Сохраняет журналы и сведения об устройстве в один ZIP, чтобы приложить его к сообщению об ошибке.",
    ),
    ("Export", "Экспорт"),
    ("Record performance", "Запись производительности"),
    (
        "Records frame times for up to 30 minutes, to show developers where the game slows down.",
        "Записывает время кадров до 30 минут, чтобы показать разработчикам, где игра тормозит.",
    ),
    ("GPU profiling", "Профилирование GPU"),
    (
        "Adds to the recording how long the GPU spends on each stage. May cost a little FPS.",
        "Добавляет в запись, сколько времени видеокарта тратит на каждый этап. Может немного снизить FPS.",
    ),
    ("Raw performance data", "Сырые данные производительности"),
    (
        "Exports the recorded measurements as JSONL files for your own analysis.",
        "Выгружает записанные замеры файлами JSONL для собственного анализа.",
    ),
    ("Game files", "Файлы игры"),
    (
        "Opens the game folder (worlds, mods, packs, reports) in the Files app.",
        "Открывает папку игры (миры, моды, наборы, отчёты) в приложении «Файлы».",
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
    ("Game resolution", "Разрешение игры"),
    (
        "The share of the screen's resolution the game draws at. Lower runs faster but looks blurrier; menus keep a usable size.",
        "Доля разрешения экрана, в которой рисуется игра. Ниже — быстрее, но картинка мыльнее; меню остаются удобного размера.",
    ),
    ("Rendering mode", "Режим рендера"),
    (
        "How the game draws through your phone's graphics driver. Applies the next time you press Play.",
        "Как игра рисует через графический драйвер телефона. Применится при следующем нажатии «Играть».",
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
    ("Home screen", "Главный экран"),
    ("Profile above Play", "Профиль над «Играть»"),
    (
        "Shows the profile picker above Play when the phone is held upright.",
        "Показывать выбор профиля над кнопкой «Играть» в вертикальной ориентации.",
    ),
    ("Last world and quick join", "Последний мир и быстрый вход"),
    (
        "Shows the last world and server quick join above Play when the phone is held upright.",
        "Показывать карточку последнего мира и быстрого входа на сервер над «Играть» в вертикальной ориентации.",
    ),
    (
        "{} results - select Install to check compatibility",
        "Найдено: {} — «Установить» проверит совместимость",
    ),
    (
        "Searching... You can switch tabs while results load.",
        "Ищем… Можно переключать вкладки, пока грузятся результаты.",
    ),
    (
        "{} .hmod / {} source-only .hnmod (not supported yet)",
        "{} .hmod / {} .hnmod только в исходниках (пока не поддерживаются)",
    ),
    (
        "{} Java mods - browsing only; this Rust client cannot run Java loaders",
        "Java-модов: {} — только просмотр, этот клиент на Rust не запускает Java-загрузчики",
    ),
    (
        "No Horizon shader packs match this search. Try another query or import a .hshader.",
        "Шейдеров Horizon по запросу нет. Попробуйте другой запрос или импортируйте .hshader.",
    ),
    (
        "Press Import .hmod to download this HTTPS link",
        "Нажмите «Импорт .hmod», чтобы скачать по этой ссылке",
    ),
    (
        "Selection saved for the next game launch",
        "Выбор сохранён до следующего запуска игры",
    ),
    (
        "Sandboxed code mods require a 64-bit APK",
        "Модам с кодом нужен 64-битный APK",
    ),
    (
        "Downloading sandboxed .hmod over HTTPS...",
        "Скачиваем .hmod по HTTPS…",
    ),
    (
        "Downloading experimental shader ZIP...",
        "Скачиваем ZIP-шейдер (экспериментально)…",
    ),
    (
        "Installed {} item(s). Open Play to load this profile.",
        "Установлено: {}. Нажмите «Играть», чтобы загрузить профиль.",
    ),
    (
        "Chunk worker ceiling saved. Applies on the next Play; other engine threads are unchanged.",
        "Лимит потоков чанков сохранён. Применится при следующем запуске.",
    ),
    (
        "Disposable cache budget saved. Applies on next Play; not a total RAM or VRAM limit.",
        "Объём кеша сохранён. Применится при следующем запуске.",
    ),
    (
        "Game files opened. Worlds, mods, packs and reports are available here.",
        "Файлы игры открыты: миры, моды, паки и отчёты.",
    ),
    (
        "Copy a complete world folder to back up or import. Finish copying before Play; never replace an existing world silently.",
        "Копируйте папку мира целиком, чтобы сохранить или импортировать её. Дождитесь конца копирования перед игрой.",
    ),
    (
        "Open system Files > Horizon Android.",
        "Откройте «Файлы» → Horizon Android.",
    ),
    ("Preparing Microsoft session...", "Готовим сеанс Microsoft…"),
    (
        "Hold Z to zoom; configurable in Options > Mods",
        "Удерживайте Z для приближения; настройки в «Настройки → Моды»",
    ),
    (
        "A configurable map in the screen corner",
        "Настраиваемая карта в углу экрана",
    ),
    (
        "Item recipes and uses beside the inventory",
        "Рецепты и применение предметов рядом с инвентарём",
    ),
    (
        "Configurable modules for the F3 debug screen",
        "Настраиваемые модули экрана отладки F3",
    ),
    ("Screen orientation", "Ориентация экрана"),
    (
        "The launcher and the game open in this orientation.",
        "Лаунчер и игра открываются в этой ориентации.",
    ),
    (
        "The launcher and the game use this orientation.",
        "Лаунчер и игра используют эту ориентацию.",
    ),
    ("Full screen in the game", "Полный экран в игре"),
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
