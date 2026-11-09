The first public release of **Horizon Aurora**: Minecraft: Java Edition 26.3, rewritten in Rust, on Android. Aurora is an independent fork of [Horizon](https://github.com/horizon-rust/horizon) that carries on the Horizon Android port (now [Starling](https://github.com/starling-client/starling)). This build is Horizon's nightly of 2026-11-07 with Aurora's Android host, launcher and fixes.

## What Aurora adds

- **A new launcher**: Home, Instances, Add-ons, Worlds, Skins and Settings in one pixel-art style, landscape and portrait. English and Russian, with a language switch in Settings.
- **Move from Horizon Android or Starling in one step**: Settings › Account › Move from Horizon Android copies its worlds, resource packs, shader packs, mods, settings and touch controls into a new instance. The old app is only read; accounts are signed in again.
- **Instances**: each with its own name, worlds, settings, add-ons and engine. A new instance starts with your language, controls and graphics settings.
- **Swappable engines**: build the game on a computer (for example with native mods), pack it as a `.hengine` and start it without reinstalling the app. Settings › Engines also downloads the newest dev engine or a newer release's engine, checked against the release's list.
- **Update notices**: the launcher tells you when a newer Aurora is out and opens its page (Settings › Engines › Check for updates turns it off).
- **Horizon Boost** as a Minecraft-style page: weather, texture animations, sky, fog, chunk uploads, and a heat limit that is Off, Soft, Balanced or Strong, with a frame-rate floor.
- **Touch layout editor**: custom buttons (key combinations, toggle, swipe, shapes, colours), named layouts with share codes. In the settings a swipe scrolls, a tap taps and a slider drags sideways from its track.
- **Accounts and skins**: offline, Microsoft and Ely.by accounts with quick switching; a skin and cape wardrobe with a 3D preview.
- World ZIP import and export, reports and logs through Android's file picker.

## From Horizon

Horizon's 2026-11-07 nightly: its reworked terrain passes, graphics settings screens, panorama, GPU timing and audio device handling, and the vanilla behaviour fixed since the end of October. As in Horizon, WebAssembly code mods no longer run: `.hmod` content mods install on every ABI, and native `.hnmod` mods come with an engine built with them.

## Fixes from Horizon Android 0.7.8

- A relaunch releases the previous session's resources; pausing when the app leaves the screen, and plugging in a keyboard or controller, no longer recreate the game; held input no longer sticks (KylePine, Horizon Android #211).
- Captured mouse movement is no longer lost when Android batches it (#238).
- Cached Minecraft files are checked before they are unpacked and copied (#237).

## Install

Download the APK for your device (most phones: **arm64-v8a**) and open it. On first start the app downloads Minecraft's original resources from Mojang and checks them.

| APK | Devices | Android |
|---|---|---|
| `arm64-v8a` | Almost every phone and tablet since 2017 | 9+ |
| `armeabi-v7a` | Old 32-bit phones and firmware | 8.1+ |
| `x86_64` | Emulators, Waydroid, Chromebooks | 9+ |
| `x86` | Old 32-bit emulators | 8.1+ |

Aurora is its own app (`io.github.iamfuckinggenius.aurora`): it installs beside Horizon Android and Starling. `SHA256SUMS.txt` lists the checksums of the APKs.

## Tested

Played on a Redmi 9A (armeabi-v7a, 32-bit Android 11, OpenGL ES on a PowerVR GE8320, 2 GB of memory) and started on a Galaxy S21 FE (arm64-v8a, Vulkan on an Adreno 660). The x86 and x86_64 APKs are built but not tested on a device. Known: on a phone with 2 GB of memory Android may close the game when memory runs short (close other apps, keep the render distance low); the first start on PowerVR compiles shaders for about 25 seconds, later starts take about one.

Not affiliated with or endorsed by Mojang, Microsoft, Horizon, Horizon Android or Starling. Report problems [here](https://github.com/IAmFuckingGenius/horizon-aurora/issues).

---

**По-русски.** Первый публичный релиз Horizon Aurora — самостоятельного форка Horizon, продолжающего Horizon Android (теперь Starling), на основе ночной сборки Horizon от 07.11.2026. Новый лаунчер, инстансы, сменные движки, Horizon Boost, редактор управления, аккаунты и гардероб; перенос миров, паков, модов и настроек из Horizon Android или Starling в один шаг (**Настройки › Аккаунт › Перенос из Horizon Android**); уведомления о новых версиях. Большинству телефонов нужен APK **arm64-v8a**, старым 32-битным — **armeabi-v7a**. Проверено на Redmi 9A (32 бита, OpenGL ES) и Galaxy S21 FE (Vulkan).
