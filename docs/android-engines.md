# Android engines

Horizon Android can start a game library built on a computer instead of the one
inside the APK, without reinstalling the app. Such a library is an **engine**: the
same `libhorizon_android.so` the APK carries, built from any tree, for example with
native mods compiled in. Worlds, settings, accounts and downloaded game files stay
shared between engines.

## How it starts

NativeActivity loads `libhorizon_boot.so` first (`crates/horizon-boot`,
`android.app.lib_name` in `packaging/android/AndroidManifest.xml`). The boot library:

1. takes `files/engines/launch.json` if it exists, renaming it to
   `launch.taken.json` before anything else, so a request is used once;
2. loads `files/engines/<id>/libhorizon_android.so` for the engine the request
   names, and calls its `ANativeActivity_onCreate`;
3. otherwise, or when that library cannot load, loads the APK's own
   `libhorizon_android.so`. The reason a requested engine failed goes to
   `files/engines/boot-error.txt`, and the launcher shows it.

A process holds one engine (winit allows one event loop per process), so switching
engines restarts the app: the launcher writes the request and restarts its activity.
An engine that crashes leaves no request behind; the next start uses the APK's
engine.

The engine reads `launch.taken.json` at start and opens what it asks
(`{"target": {"open": "title"}}`, `{"world": "<folder>"}` or
`{"server": "<address>"}`).

## Building and installing an engine

```sh
python3 tools/android_engine_pack.py --abi arm64-v8a --name "My build"
```

This builds the library like `tools/android_build.py --rust-only` does, strips it and
writes `.local/android/engines/<id>-<abi>.hengine`: a ZIP of `engine.json` and
`libhorizon_android.so`. Copy the file to the phone, then in the launcher open
**Settings › Engines › Import**, pick the file, and press **Start** on it.

`engine.json`:

| Field | Meaning |
|---|---|
| `format` | `1`; the launcher refuses other formats |
| `id` | The folder name on the phone: letters, digits, `-`, `_`, `.` |
| `name` | Shown in the launcher |
| `version`, `commit`, `dirty`, `built` | Where the build came from |
| `abi` | `arm64-v8a`, `armeabi-v7a`, `x86_64` or `x86`; must match the device |
| `size`, `sha256` | Of the library, checked on import |
| `shell` | The [shell level](#shell-levels) it needs |

On import the launcher checks the format, id, ABI, the ELF header's machine, the
size and the SHA-256, then installs the library read-only (Android 14 requires
code an app loads from its own files to be read-only). An engine with the same id
replaces the old one.

## Engines with native mods

Native mods (`.hnmod`, [native-mods.md](native-mods.md)) change the game's source before it is
compiled, so on Android they come as an engine built with them. The manual workflow
**Horizon Aurora native-mod engine** (`.github/workflows/android-native-engine.yml`) applies a
bundle with horizon-mixins, at the launcher's `MIXINS_REV`, then builds and packs the engine as
above for arm64-v8a and armeabi-v7a. Its `bundle` input is `smoke` (a one-hook test bundle),
`github:OWNER/REPO@<commit>[:folder]`, or a `.hnmod` URL with its `#sha256=`. The engines are run
artifacts; `publish` adds them to a rolling `engines` pre-release, which the launcher's list does
not read yet, so they are imported by hand. A mod's content (`.hmod`) goes in the same instance's
mods.

## Engines from Aurora's releases

**Settings › Engines › Engines from Aurora › Check** reads the engine lists published
with the latest release and with the rolling `dev` pre-release (`engines.json`, written by
`tools/android_engine_index.py`), and offers the engines built for this device:

- **Aurora Dev** (`aurora-dev`): the engine of the newest dev build, so a release app can
  try what is coming on its own instance;
- **Horizon Aurora** (`aurora`): the latest release's engine, for an app that has not been
  updated yet. The engine the app already carries shows as "Same as the built-in engine".

A download is checked against the list (size and SHA-256), then installed like an
imported engine (the checks below), and **Update** appears when the list has a newer
build of an installed engine. Lists and files come only from this repository's releases.

### A newer build of the app

The same lists tell the app of a newer build of itself, since every published build
carries its engine: a release looks for a release engine of a later version, a dev build
(`-dev.<label>`) for a dev engine of another commit, and a local build for nothing. The
launcher reads the lists when it opens, says so once in a notice, and shows **A newer
Horizon Aurora is out** with **Open** (the release's page; the APK installs over the app,
keeping worlds and settings) at the top of Engines from Aurora. **Check for updates** there
turns the reading at start off; the lists are then read only on **Check**.

## Shell levels

An engine relies on the APK around it: the boot library, the Java side
(`packaging/android/java`) and the manifest. `horizon_boot::SHELL` numbers what an APK
offers; the boot library exports it and `engine.json` records the level an engine was
built with (`shell`). The launcher refuses an engine with a higher level than its app's
("Update Horizon Aurora first") instead of letting it fail at start. Engines packed before
levels existed have none (0). When the Java side or the manifest changes,
`tools/test_android_engines.py` fails until `SHELL` is bumped (if engines built from the
tree need the change) and the new state is recorded with `--record-shell`
(`packaging/android/shell.json`).

## Instances

An instance is what the player plays: a name, an engine (the APK's own or an installed
one) and its own game folder with worlds, settings and add-ons. Instance 0 is the main
game folder; instance N is `profiles/profile-N` (older launcher profiles show up as
instances). Names and engines are kept in `files/instances.json`. A new instance starts
with the current one's settings (language, controls, video, Horizon's graphics), not its
worlds or add-ons.

Play on an instance whose engine is not the one running restarts the app on its engine
with `{"target": {"instance": N, ...}}`; the engine makes instance N the one in use and
opens the target. For the APK's own engine the request names the engine `builtin`,
which no installed engine may take. Deleting an engine moves its instances back to the
APK's engine.

## Compatibility and trust

An engine runs with the app's full permissions and data, like the APK itself. Only
install engines you built or trust. An engine built from a much older or newer tree
may not understand the launcher's files or the installed game resources; build
engines from the same release line as the APK.

Tests: `python3 tools/test_android_engines.py` (install checks and the boot
library's request handling, on the host).
