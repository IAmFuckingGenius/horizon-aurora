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
/// The player's choice in Settings: a language, or Android's again.
pub(crate) fn choose(choice: crate::catalog::LauncherLanguage) {
    use crate::catalog::LauncherLanguage;
    match choice {
        LauncherLanguage::Auto => LANG.store(0, Ordering::Relaxed),
        LauncherLanguage::English => set(Lang::En),
        LauncherLanguage::Russian => set(Lang::Ru),
    }
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
    ("Content mods (.hmod)", "Контентные моды (.hmod)"),
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
        "Downloading .hmod over HTTPS...",
        "Скачиваем .hmod по HTTPS…",
    ),
    (
        "A newer Horizon Aurora is out",
        "Вышла новая версия Horizon Aurora",
    ),
    (
        "Install its APK over this app: worlds and settings stay.",
        "Установите её APK поверх этого приложения: миры и настройки сохранятся.",
    ),
    ("Check for updates", "Проверять обновления"),
    (
        "Reads the list of Aurora's latest release on GitHub when the launcher opens.",
        "При запуске лаунчер читает список последнего релиза Aurora на GitHub.",
    ),
    ("A new dev build is out", "Вышла новая dev-сборка"),
    ("Move from Horizon Android", "Перенос из Horizon Android"),
    ("Older servers", "Старые серверы"),
    (
        "Joins servers of Minecraft 1.20.2 to 26.2 through Horizon's protocol adapter, chosen by the server's version. Experimental: some items and features of older releases may not work.",
        "Подключение к серверам Minecraft от 1.20.2 до 26.2 через адаптер протокола Horizon, по версии сервера. Экспериментально: часть предметов и функций старых версий может не работать.",
    ),
    (
        "Copies worlds, packs, shaders, mods, settings and touch controls from Horizon Android or Starling into a new instance. In the list of places choose «Horizon Android», then Use this folder. Accounts stay there: sign in again.",
        "Копирует миры, ресурспаки, шейдеры, моды, настройки и сенсорное управление из Horizon Android или Starling в новый инстанс. В списке мест выберите «Horizon Android», затем «Использовать эту папку». Аккаунты туда не переносятся: войдите снова.",
    ),
    ("Move", "Перенести"),
    (
        "Moving from Horizon Android",
        "Переносим из Horizon Android",
    ),
    (
        "Reading Horizon Android's folders",
        "Читаем папки Horizon Android",
    ),
    (
        "Copying worlds, packs and settings",
        "Копируем миры, паки и настройки",
    ),
    (
        "Moved from Horizon Android",
        "Перенесено из Horizon Android",
    ),
    ("Move cancelled", "Перенос отменён"),
    (
        "Horizon Android could not share its files: {}. Close its game and try again.",
        "Horizon Android не смог отдать файлы: {}. Закройте его игру и попробуйте снова.",
    ),
    (
        "Too many files to move at once",
        "Слишком много файлов для одного переноса",
    ),
    (
        "Could not move from Horizon Android",
        "Не удалось перенести из Horizon Android",
    ),
    (
        "This folder is not Horizon Android's. Choose «Horizon Android» in the list of places.",
        "Это не папка Horizon Android. Выберите «Horizon Android» в списке мест.",
    ),
    (
        "Choose Horizon Android's own folder, or one of Worlds, Resource Packs, Shader Packs, Mods and Configuration in it.",
        "Выберите саму папку Horizon Android или одну из папок в ней: Worlds, Resource Packs, Shader Packs, Mods, Configuration.",
    ),
    (
        "Instance «{}»: {} world(s). Choose it on Home or under Instances, and sign in to your accounts again.",
        "Инстанс «{}», миров: {}. Выберите его на главной или в «Инстансах» и снова войдите в аккаунты.",
    ),
    ("Horizon Aurora {} is out", "Вышла Horizon Aurora {}"),
    (
        "Settings › Engines: open its page to update.",
        "«Настройки › Движки»: откройте страницу релиза, чтобы обновиться.",
    ),
    (
        "Installed {}. Turn it on in Installed.",
        "Установлен {}. Включите его в разделе «Установлено».",
    ),
    (
        "This .hnmod is a native mod: build an engine with it on a computer and import it in Settings › Engines. .hmod content mods install here.",
        "Это нативный мод .hnmod: соберите с ним движок на компьютере и импортируйте его в «Настройки › Движки». Контентные моды .hmod ставятся здесь.",
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
    // World and report transfers (transfers.rs, world_archive.rs).
    ("Import world", "Импорт мира"),
    (
        "Press Play, then Singleplayer, to create one, or bring one in with Import world. Copied world folders appear here too.",
        "Нажмите «Играть», затем «Одиночная игра», чтобы создать мир, или добавьте готовый через «Импорт мира». Скопированные папки миров тоже появятся здесь.",
    ),
    ("Exporting the world", "Экспорт мира"),
    ("Importing a world", "Импорт мира"),
    ("Saving the file", "Сохранение файла"),
    ("Stopping and cleaning up", "Остановка и очистка"),
    ("Copying the ZIP into Horizon", "Копирование ZIP в Horizon"),
    ("Checking the ZIP", "Проверка ZIP"),
    ("Packing the world into a ZIP", "Упаковка мира в ZIP"),
    ("Unpacking the world", "Распаковка мира"),
    ("Writing the file", "Запись файла"),
    ("{} of {} files", "{} из {} файлов"),
    ("World exported", "Мир экспортирован"),
    ("Export cancelled", "Экспорт отменён"),
    (
        "Could not export the world",
        "Не удалось экспортировать мир",
    ),
    ("World imported", "Мир импортирован"),
    ("Import cancelled", "Импорт отменён"),
    ("Could not import the world", "Не удалось импортировать мир"),
    ("File saved", "Файл сохранён"),
    ("Saving cancelled", "Сохранение отменено"),
    ("Could not save the file", "Не удалось сохранить файл"),
    ("Could not share the file", "Не удалось поделиться файлом"),
    ("«{}» is now in Worlds.", "«{}» теперь в разделе «Миры»."),
    ("Saved as «{}».", "Сохранено как «{}»."),
    ("Nothing was added.", "Ничего не добавлено."),
    (
        "The unfinished file was removed.",
        "Недописанный файл удалён.",
    ),
    (
        "Not enough free space on the device.",
        "На устройстве недостаточно свободного места.",
    ),
    (
        "Not enough free space: free up {} MiB more and try again.",
        "Недостаточно места: освободите ещё {} МиБ и попробуйте снова.",
    ),
    (
        "The file is not a ZIP archive, or it is damaged.",
        "Это не ZIP-архив, или он повреждён.",
    ),
    (
        "No world in this ZIP: level.dat must be at its root or in one folder inside it.",
        "В этом ZIP нет мира: level.dat должен лежать в корне архива или в одной папке внутри него.",
    ),
    (
        "The ZIP holds more than one world. Put each world in its own ZIP.",
        "В ZIP больше одного мира. Упакуйте каждый мир в отдельный ZIP.",
    ),
    (
        "The ZIP has a file path that leads outside the world folder, so nothing was unpacked.",
        "В ZIP есть путь, который ведёт за пределы папки мира, поэтому ничего не распаковано.",
    ),
    (
        "The ZIP has a symbolic link. Worlds never contain them, so nothing was unpacked.",
        "В ZIP есть символическая ссылка. В мирах их не бывает, поэтому ничего не распаковано.",
    ),
    (
        "The ZIP lists the same file twice, so nothing was unpacked.",
        "В ZIP один и тот же файл указан дважды, поэтому ничего не распаковано.",
    ),
    (
        "The ZIP is password-protected. Pack the world without a password.",
        "ZIP защищён паролем. Упакуйте мир без пароля.",
    ),
    (
        "The ZIP uses a compression method Horizon cannot read. Pack it as a regular ZIP.",
        "ZIP сжат способом, который Horizon не читает. Упакуйте его как обычный ZIP.",
    ),
    (
        "The ZIP has too many files to be one world.",
        "В ZIP слишком много файлов для одного мира.",
    ),
    (
        "The ZIP is damaged: a file is not the size it claims.",
        "ZIP повреждён: размер файла не совпадает с указанным.",
    ),
    (
        "The world has too many files for one ZIP",
        "В мире слишком много файлов для одного ZIP",
    ),
    ("The world folder is missing", "Папка мира не найдена"),
    (
        "Too many worlds with this name",
        "Слишком много миров с таким именем",
    ),
    (
        "Finish the add-on import first",
        "Сначала дождитесь импорта дополнения",
    ),
    (
        "Wait for the current transfer to finish",
        "Дождитесь окончания текущей передачи",
    ),
    (
        "The system file picker is not available",
        "Системный выбор файлов недоступен",
    ),
    (
        "Could not open the file picker",
        "Не удалось открыть выбор файлов",
    ),
    (
        "Android has no app to choose files with. Use Open in Files instead.",
        "В Android нет приложения для выбора файлов. Воспользуйтесь «Открыть в Файлах».",
    ),
    ("Transfer interrupted", "Передача прервана"),
    (
        "Android closed Horizon while the file picker was open. Try again.",
        "Android закрыл Horizon, пока был открыт выбор файлов. Попробуйте ещё раз.",
    ),
    ("Could not open the file", "Не удалось открыть файл"),
    (
        "Android returned an unsupported location.",
        "Android вернул неподдерживаемое расположение.",
    ),
    (
        "Android did not give access to the chosen file.",
        "Android не дал доступа к выбранному файлу.",
    ),
    (
        "Horizon Android report for developers",
        "Отчёт Horizon Android для разработчиков",
    ),
    (
        "Horizon Android performance data",
        "Данные производительности Horizon Android",
    ),
    (
        "Horizon Android launch log",
        "Журнал запуска Horizon Android",
    ),
    (
        "No app can receive this file",
        "Нет приложения, которое может принять этот файл",
    ),
    ("Share", "Поделиться"),
    ("Share as text", "Поделиться текстом"),
    ("Save to a folder", "Сохранить в папку"),
    ("Save as a text file", "Сохранить как текстовый файл"),
    (
        "Pick a folder or drive with the system file picker.",
        "Выберите папку или диск в системном окне выбора файлов.",
    ),
    (
        "Send it to another app: mail, a messenger or cloud storage.",
        "Отправьте в другое приложение: почту, мессенджер или облако.",
    ),
    (
        "Shows Horizon Android › Reports in the Files app.",
        "Открывает Horizon Android › Отчёты в приложении «Файлы».",
    ),
    (
        "Logs and device details in one ZIP. Nothing is uploaded by itself: review the ZIP before you send it.",
        "Журналы и сведения об устройстве в одном ZIP. Сам он никуда не отправляется: просмотрите ZIP перед отправкой.",
    ),
    (
        "The latest recording as a JSONL file, for your own analysis or for developers.",
        "Последняя запись в файле JSONL — для собственного анализа или для разработчиков.",
    ),
    (
        "The errors and logs of the last launches as one text file. Passwords, tokens and addresses are removed; review it before you send it.",
        "Ошибки и журналы последних запусков одним текстовым файлом. Пароли, токены и адреса удалены; просмотрите файл перед отправкой.",
    ),
    (
        "The errors and logs of the last game launches. Read the summary, save it as a text file or share it.",
        "Ошибки и журналы последних запусков игры. Прочитайте сводку, сохраните её текстовым файлом или поделитесь ею.",
    ),
    (
        "The previous session ended without a clean exit (last phase: {}). The game may have crashed, or Android closed it.",
        "Прошлый сеанс завершился некорректно (последняя фаза: {}). Возможно, игра вылетела или её закрыл Android.",
    ),
    (
        "The previous session ended normally.",
        "Прошлый сеанс завершился нормально.",
    ),
    ("Last error: {}", "Последняя ошибка: {}"),
    ("Preparing the report", "Подготовка отчёта"),
    (
        "Nothing is uploaded. Review the ZIP before you send it.",
        "Ничего не отправляется. Просмотрите ZIP перед отправкой.",
    ),
    (
        "Could not prepare the report",
        "Не удалось подготовить отчёт",
    ),
    ("No recording yet", "Записи пока нет"),
    (
        "No performance report. Enable recording, then launch and play first.",
        "Отчёта о производительности нет. Включите запись, затем запустите игру и поиграйте.",
    ),
    (
        "Could not write the launch log",
        "Не удалось записать журнал запуска",
    ),
    ("Opened in Files", "Открыто в «Файлах»"),
    ("Open the Files app", "Откройте приложение «Файлы»"),
    (
        "«{}» is in Horizon Android › Reports.",
        "«{}» лежит в Horizon Android › Отчёты.",
    ),
    (
        "Reports opened in Files. Copy or share a report from there.",
        "Отчёты открыты в «Файлах». Скопируйте отчёт или поделитесь им оттуда.",
    ),
    (
        "Open the system Files app, then Horizon Android / Reports.",
        "Откройте системное приложение «Файлы», затем Horizon Android / Отчёты.",
    ),
    (
        "Android could not open the chosen file ({}). Try it from another place, such as Downloads.",
        "Android не смог открыть выбранный файл ({}). Попробуйте выбрать его из другого места, например из «Скачанных».",
    ),
    ("Saved accounts", "Сохранённые аккаунты"),
    ("Ely.by account", "Аккаунт Ely.by"),
    ("Play without signing in", "Игра без входа"),
    ("Microsoft account", "Аккаунт Microsoft"),
    ("Now playing as {} ({})", "Сейчас вы играете как {} ({})"),
    (
        "Now playing as {} (offline)",
        "Сейчас вы играете как {} (без входа)",
    ),
    (
        "Any name. Singleplayer and servers without licence checks.",
        "Любое имя. Одиночная игра и серверы без проверки лицензии.",
    ),
    (
        "A licensed Minecraft account. Needed for most public servers.",
        "Лицензионный аккаунт Minecraft. Нужен для большинства публичных серверов.",
    ),
    (
        "A free account with skins, for servers that use Ely.by.",
        "Бесплатный аккаунт со скинами для серверов, которые работают с Ely.by.",
    ),
    (
        "Switch between accounts signed in before: {}.",
        "Переключение между аккаунтами, в которые вы уже входили: {}.",
    ),
    (
        "No saved accounts yet. Sign in with Microsoft or Ely.by and the account stays here.",
        "Сохранённых аккаунтов пока нет. Войдите через Microsoft или Ely.by, и аккаунт появится здесь.",
    ),
    ("Previous", "Назад"),
    ("Next", "Дальше"),
    ("Username or email", "Логин или почта"),
    ("Password (not saved)", "Пароль (не сохраняется)"),
    (
        "Two-factor code, if you set one up",
        "Код двухфакторной защиты, если она включена",
    ),
    (
        "Your password goes only to authserver.ely.by and is not stored on the phone.",
        "Пароль отправляется только на authserver.ely.by и не хранится на телефоне.",
    ),
    ("Done", "Готово"),
    ("Sign in to Ely.by", "Войти в Ely.by"),
    (
        "Singleplayer worlds and servers without licence checks. The name is up to 16 letters, digits or _.",
        "Одиночные миры и серверы без проверки лицензии. Имя — до 16 латинских букв, цифр или _.",
    ),
    ("Play offline", "Играть без входа"),
    ("Skins", "Скины"),
    ("Capes", "Плащи"),
    ("Skins & capes", "Скины и плащи"),
    ("Import PNG", "Добавить PNG"),
    ("On the account", "В аккаунте"),
    (
        "On the account · tap to wear it",
        "В аккаунте · нажмите, чтобы надеть",
    ),
    ("On this phone", "На этом телефоне"),
    ("Model: Alex (slim arms)", "Модель: Алекс (тонкие руки)"),
    ("Model: Steve (classic)", "Модель: Стив (обычная)"),
    (
        "Use the account's own appearance",
        "Вернуть облик из аккаунта",
    ),
    ("Refresh from the account", "Обновить из аккаунта"),
    (
        "Upload the chosen skin to Microsoft",
        "Загрузить выбранный скин в Microsoft",
    ),
    (
        "Change the Ely.by skin on the website",
        "Сменить скин Ely.by на сайте",
    ),
    ("Back to the default look", "Вернуть стандартный облик"),
    ("Hide or show the cape", "Скрыть или показать плащ"),
    (
        "Cloud profile synchronized. Local overrides are kept separate.",
        "Профиль из аккаунта обновлён. Ваш выбор на телефоне сохранён отдельно.",
    ),
    (
        "Applied locally. The account's cloud profile is unchanged.",
        "Применено на этом телефоне. В аккаунте ничего не изменилось.",
    ),
    (
        "Choose a PNG image in Android Files.",
        "Выберите картинку PNG в файлах Android.",
    ),
    (
        "Using the account's cloud appearance. No cloud changes were made.",
        "Используется облик из аккаунта. В аккаунте ничего не изменено.",
    ),
    (
        "Local override cleared. Using the default appearance.",
        "Выбор на телефоне сброшен. Используется стандартный облик.",
    ),
    (
        "Offline appearance is local to this installation.",
        "Облик без входа хранится только на этом телефоне.",
    ),
    (
        "Ely.by cloud skins are managed at ely.by. Custom capes can be applied locally.",
        "Скин Ely.by меняется на сайте ely.by. Свои плащи можно надеть на телефоне.",
    ),
    (
        "Select a local skin before uploading to your Microsoft profile.",
        "Сначала выберите скин на телефоне, потом загружайте его в Microsoft.",
    ),
    (
        "Contacting account service...",
        "Связываемся с сервисом аккаунтов…",
    ),
    (
        "Sign in before synchronizing your cloud appearance.",
        "Войдите в аккаунт, чтобы обновить облик из него.",
    ),
    (
        "The selected account changed. Open the gallery again.",
        "Выбранный аккаунт сменился. Откройте вкладку скинов заново.",
    ),
    (
        "Signed out. Offline play is available.",
        "Вы вышли из аккаунта. Можно играть без входа.",
    ),
    (
        "Code copied. Paste it on the Microsoft page.",
        "Код скопирован. Вставьте его на странице Microsoft.",
    ),
    (
        "Could not copy the code. Try again.",
        "Не удалось скопировать код. Попробуйте ещё раз.",
    ),
    ("Signed in as {}", "Вы вошли как {}"),
    ("Engines", "Движки"),
    ("Installing an engine", "Установка движка"),
    (
        "Copying the engine into Horizon",
        "Копируем движок в Horizon",
    ),
    (
        "Checking and installing the engine",
        "Проверяем и устанавливаем движок",
    ),
    ("Engine installed", "Движок установлен"),
    ("Engine import cancelled", "Установка движка отменена"),
    (
        "Could not install the engine",
        "Не удалось установить движок",
    ),
    (
        "«{}» is in Settings › Engines. Start it from there.",
        "«{}» теперь в Настройки › Движки. Запускайте его оттуда.",
    ),
    (
        "Engine in use: built into the app",
        "Сейчас работает встроенный движок",
    ),
    ("Engine in use: «{}»", "Сейчас работает движок «{}»"),
    (
        "Started for this launch only. The next start of the app uses the built-in engine again.",
        "Он запущен только на этот раз. При следующем запуске приложения снова будет встроенный движок.",
    ),
    ("Built-in engine", "Встроенный движок"),
    ("Add an engine", "Добавить движок"),
    (
        "An engine is the game built on a computer, with your native mods compiled in, and packed into a .hengine file by tools/android_engine_pack.py. It runs without reinstalling the app; your worlds, settings and account stay shared.",
        "Движок — это игра, собранная на компьютере вместе с вашими нативными модами и упакованная в файл .hengine скриптом tools/android_engine_pack.py. Он запускается без переустановки приложения; миры, настройки и аккаунт остаются общими.",
    ),
    ("Import", "Импорт"),
    ("uncommitted changes", "есть незакоммиченные правки"),
    ("Start", "Запустить"),
    ("Tap again", "Нажмите ещё раз"),
    ("Delete", "Удалить"),
    ("This engine is running", "Этот движок сейчас работает"),
    (
        "Switch to the built-in engine first, then delete it.",
        "Сначала переключитесь на встроенный движок, потом удаляйте.",
    ),
    ("«{}» deleted", "«{}» удалён"),
    ("Could not delete the engine", "Не удалось удалить движок"),
    ("Could not start the engine", "Не удалось запустить движок"),
    (
        "Could not restart Horizon",
        "Не удалось перезапустить Horizon",
    ),
    (
        "Close the app and open it again to switch the engine.",
        "Закройте приложение и откройте снова, чтобы сменить движок.",
    ),
    ("Engine «{}» did not start", "Движок «{}» не запустился"),
    (
        "The built-in engine started instead. Android said: {}",
        "Вместо него запущен встроенный движок. Ответ Android: {}",
    ),
    (
        "No capes on this phone yet. Add a cape PNG (64×32) with the button above; it shows only for you.",
        "На телефоне пока нет плащей. Добавьте PNG плаща (64×32) кнопкой выше; его будете видеть только вы.",
    ),
    (
        "No skins on this phone yet. Add a skin PNG (64×64) with the button above, then tap it to wear it.",
        "На телефоне пока нет скинов. Добавьте PNG скина (64×64) кнопкой выше и нажмите на него, чтобы надеть.",
    ),
    (
        "This file is not an engine: it is not a ZIP archive.",
        "Это не движок: файл не является ZIP-архивом.",
    ),
    (
        "This file is not an engine: it has no engine.json.",
        "Это не движок: в нём нет engine.json.",
    ),
    (
        "The engine's engine.json cannot be read.",
        "Не удаётся прочитать engine.json движка.",
    ),
    (
        "This engine was packed for another launcher version. Pack it again with this tree's tools/android_engine_pack.py.",
        "Движок упакован для другой версии лаунчера. Упакуйте его заново скриптом tools/android_engine_pack.py из этой версии.",
    ),
    (
        "The engine's id is not a valid folder name.",
        "Имя (id) движка не подходит для папки.",
    ),
    (
        "The engine library has an impossible size.",
        "У библиотеки движка невозможный размер.",
    ),
    (
        "The engine has no valid checksum.",
        "У движка нет правильной контрольной суммы.",
    ),
    (
        "The engine library is larger than its manifest says.",
        "Библиотека движка больше, чем указано в его описании.",
    ),
    (
        "The engine library is incomplete.",
        "Библиотека движка неполная.",
    ),
    (
        "The engine library is damaged: its checksum does not match.",
        "Библиотека движка повреждена: контрольная сумма не совпадает.",
    ),
    (
        "The engine library is not a library for this device.",
        "Библиотека движка не подходит для этого устройства.",
    ),
    (
        "This engine is not installed.",
        "Этот движок не установлен.",
    ),
    ("Instances", "Инстансы"),
    ("Instance", "Инстанс"),
    ("New instance", "Новый инстанс"),
    (
        "Each has its own worlds, settings and add-ons, and its engine",
        "У каждого свои миры, настройки, дополнения и движок",
    ),
    ("Change", "Изменить"),
    ("built-in engine", "встроенный движок"),
    ("engine «{}»", "движок «{}»"),
    ("engine «{}» is not installed", "движок «{}» не установлен"),
    ("played {}", "играли {}"),
    ("in use", "выбран"),
    ("Engine", "Движок"),
    ("Instance name", "Название инстанса"),
    ("Save", "Сохранить"),
    ("Create", "Создать"),
    (
        "Tap again: its worlds go too",
        "Ещё раз: миры тоже удалятся",
    ),
    (
        "Could not save the instance",
        "Не удалось сохранить инстанс",
    ),
    (
        "Could not delete the instance",
        "Не удалось удалить инстанс",
    ),
    ("Instance «{}»", "Инстанс «{}»"),
    ("Instance {}", "Инстанс {}"),
    (
        "Instance «{}». Last world: «{}», {}.",
        "Инстанс «{}». Последний мир — «{}», {}.",
    ),
    (
        "Instance «{}». No worlds yet: create one from Play.",
        "Инстанс «{}». Миров пока нет: создайте мир через «Играть».",
    ),
    (
        "Minecraft {} · instance «{}»",
        "Minecraft {} · инстанс «{}»",
    ),
    (
        "Each instance keeps its own worlds, settings and add-ons, and can run on its own engine. Manage them on the Instances page.",
        "У каждого инстанса свои миры, настройки и дополнения, и он может работать на своём движке. Управлять ими можно на странице «Инстансы».",
    ),
    ("Choose an instance", "Выберите инстанс"),
    (
        "Each instance keeps its own worlds, settings and add-ons, and can run on its own engine.",
        "У каждого инстанса свои миры, настройки и дополнения, и он может работать на своём движке.",
    ),
    (
        "Its own worlds, settings and add-ons, and an engine of its choice",
        "Свои миры, настройки, дополнения и движок на выбор",
    ),
    (
        "The main instance cannot be deleted.",
        "Основной инстанс удалить нельзя.",
    ),
    ("Too many instances", "Слишком много инстансов"),
    ("Automatic", "Авто"),
    (
        "The launcher's language. Automatic follows Android's. The game has its own in its Options.",
        "Язык лаунчера. «Авто» берёт язык Android. У игры свой язык — в её настройках.",
    ),
    ("Install an engine?", "Установить движок?"),
    (
        "An engine is the game's code. It runs with all of Horizon's rights: your worlds, accounts and files. Install only an engine you built yourself or got from someone you trust.",
        "Движок — это код игры. Он работает со всеми правами Horizon: доступ к вашим мирам, аккаунтам и файлам. Ставьте только движок, который собрали сами или получили от того, кому доверяете.",
    ),
    ("Choose the file", "Выбрать файл"),
    ("Engines from Aurora", "Движки от Aurora"),
    (
        "Official builds from Horizon Aurora's releases. The dev engine has new features before a release and may be unstable: run it on its own instance.",
        "Официальные сборки из релизов Horizon Aurora. В dev-движке новые функции появляются раньше релиза, и он может быть нестабильным: запускайте его на отдельном инстансе.",
    ),
    ("Check", "Проверить"),
    ("Check again", "Проверить снова"),
    ("Checking…", "Проверка…"),
    ("Download", "Скачать"),
    ("Update", "Обновить"),
    ("Downloading… {}%", "Загрузка… {}%"),
    (
        "Could not load the list of engines",
        "Не удалось загрузить список движков",
    ),
    (
        "No engines for this device yet",
        "Для этого устройства движков пока нет",
    ),
    (
        "Same as the built-in engine",
        "Совпадает со встроенным движком",
    ),
    (
        "Needs a newer version of the app. Update Horizon Aurora first.",
        "Нужна более новая версия приложения. Сначала обновите Horizon Aurora.",
    ),
    (
        "This engine needs a newer version of the app. Update Horizon Aurora first.",
        "Этому движку нужна более новая версия приложения. Сначала обновите Horizon Aurora.",
    ),
    ("«{}» installed", "«{}» установлен"),
    (
        "Start it below, or choose it for an instance.",
        "Запустите его ниже или выберите для инстанса.",
    ),
    (
        "The list of engines cannot be read.",
        "Список движков не читается.",
    ),
    (
        "The list of engines is for another version of the app.",
        "Список движков сделан для другой версии приложения.",
    ),
    (
        "The engine file is larger than its list says.",
        "Файл движка больше, чем указано в списке.",
    ),
    (
        "The download is incomplete. Try again.",
        "Загрузка не завершилась. Попробуйте ещё раз.",
    ),
    (
        "The engine file is damaged: its checksum does not match.",
        "Файл движка повреждён: контрольная сумма не совпадает.",
    ),
    (
        "The engine file is not the engine the list names.",
        "В файле не тот движок, что указан в списке.",
    ),
    (
        "The download stopped unexpectedly.",
        "Загрузка неожиданно прервалась.",
    ),
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
