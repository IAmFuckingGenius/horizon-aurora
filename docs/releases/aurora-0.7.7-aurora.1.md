The first release of **Horizon Aurora**: Minecraft: Java Edition 26.3, rewritten in Rust, on Android. Aurora is an independent fork of [Horizon](https://github.com/horizon-rust/horizon) that carries on the Horizon Android port (now merged into [Starling](https://github.com/starling-client/starling)): it is built on Horizon Android 0.7.7 with the fixes of 0.7.8.

## What Aurora adds

- **A new launcher**: Home, Instances, Add-ons, Worlds, Skins and Settings in one pixel-art style, landscape and portrait. English and Russian, with a language switch in Settings.
- **Instances**: each with its own name, worlds, settings, add-ons and engine. A new instance starts with your language, controls and graphics settings.
- **Swappable engines**: build the game on a computer (for example with native mods), pack it as a `.hengine` and start it without reinstalling the app. The launcher asks before an import; an engine that fails to start falls back to the built-in one and says why.
- **Engines from Aurora**: Settings › Engines downloads the newest dev engine (new features before a release, on its own instance) or a newer release's engine, checked against the release's list, without reinstalling the app.
- **Horizon Boost** as a Minecraft-style page: weather, texture animations, sky, fog, chunk uploads, and a heat limit that is Off, Soft, Balanced or Strong, with a frame-rate floor.
- **Touch layout editor**: custom buttons (key combinations, toggle, swipe, shapes, colours), where each control shows, named layouts with share codes.
- **Accounts and skins**: offline, Microsoft and Ely.by accounts with quick switching; a skin and cape wardrobe with a 3D preview.
- World ZIP import and export, reports and logs through Android's file picker.

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

Aurora is its own app (`io.github.iamfuckinggenius.aurora`): it installs beside Horizon Android and does not see its data. To bring worlds over, export them as ZIPs in Horizon Android and import them under **Worlds › Import world**; sign in to accounts again. `SHA256SUMS.txt` lists the checksums of the APKs.

## Tested

The arm64-v8a build was played on a phone (Vulkan); the launcher, instances, engines and file import/export were also checked in Waydroid (x86_64). The armeabi-v7a and x86 APKs are built and audited but not tested on a device. Emulator frame rates say nothing about phones.

Not affiliated with or endorsed by Mojang, Microsoft, Horizon or Horizon Android. Report problems [here](https://github.com/IAmFuckingGenius/horizon-aurora/issues).

---

**По-русски.** Первый релиз Horizon Aurora — самостоятельного форка Horizon, продолжающего Horizon Android (основа 0.7.7 с исправлениями 0.7.8): новый лаунчер, инстансы, сменные движки, Horizon Boost, редактор управления, аккаунты и гардероб. Большинству телефонов нужен APK **arm64-v8a**. Aurora — отдельное приложение: миры из Horizon Android переносятся экспортом в ZIP и импортом в **Миры › Импорт мира**, в аккаунты нужно войти заново.
