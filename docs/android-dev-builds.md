# Development APKs

The **Horizon Aurora dev APKs** Actions workflow builds ARM64 on code pushes to `main`.
Documentation-only pushes are skipped. Older in-progress builds are cancelled
when a newer commit arrives. **Run workflow → all_abis** builds all four ABIs.
Pull requests never receive the signing key or trigger this workflow.

These APKs use the optimized Rust `release` profile (including ThinLTO), public
payload checks and official resource downloads. The package is
`io.github.iamfuckinggenius.aurora.dev` ("Aurora Dev" on the phone), beside the release
app ([releases](android-releases.md)); the visible version contains the run number and
source commit. Each ARM64 build replaces the **`dev` pre-release** on the Releases page, so the
newest APK downloads on a phone without signing in. Other ABIs stay run artifacts.

The version code is the number of minutes since 2020 for CI and for local builds alike
(`tools/android_build.py` passes it), so the newer APK installs over the older one
whichever made it, as long as both are signed with the same key.

## One-time maintainer setup

The workflow signs with `ANDROID_DEV_KEYSTORE_BASE64` (repository Settings → Secrets
and variables → Actions). This fork uses the local development key,
`.local/android/debug.keystore` (created by `tools/android_build.py`; password and
alias `android` / `androiddebugkey`), so CI builds update locally built dev APKs in
place:

```sh
base64 -w0 .local/android/debug.keystore | gh secret set ANDROID_DEV_KEYSTORE_BASE64
```

Keep a private backup of that keystore outside the repository: losing it means the
next dev APK needs a reinstall (and worlds exported first). Never attach the key to an
issue, artifact, release or message. It is a development key, not a release key.

## Sharing with testers

The simplest link is the `dev` pre-release on the Releases page: its ARM64 APK
downloads without signing in. Run artifacts (Actions → Horizon Aurora dev APKs → the
run → Artifacts) also hold the other ABIs; GitHub requires sign-in to download them,
and they expire after 14 days. Each contains the APK, checksums,
license notices and `BUILD.json` identifying the exact source and build profile.
Experimental builds have not automatically passed gameplay or performance tests.

Private signing keys, raw Minecraft inputs, prepared resource directories,
player data and local diagnostics are not uploaded as downloadable artifacts.
Only Cargo/Gradle build caches are retained separately for subsequent CI builds.
Cold builds also prepare the generated engine tables, so they may take
substantially longer than an incremental local build.
