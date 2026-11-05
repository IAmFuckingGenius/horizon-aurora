# Horizon Aurora releases

Two channels, two apps that install side by side:

| Channel | Package | Name on the phone | Signed with | Where |
|---|---|---|---|---|
| Release | `io.github.iamfuckinggenius.aurora` | Horizon Aurora | the release key | a GitHub release per version, four ABIs |
| Dev | `io.github.iamfuckinggenius.aurora.dev` | Aurora Dev | the development key | the rolling `dev` pre-release ([dev builds](android-dev-builds.md)) and local builds |

The version is `versionName` in `packaging/android/build.gradle` (`0.7.7-aurora.1`: the
Horizon Android version it builds on, then Aurora's own release number). The launcher
shows it; dev builds add `-dev.<run>.<commit>`. Version codes count minutes since 2020:
a release takes its tagged commit's time, so every release is above the previous one.

## One-time setup: the release key

The release key signs every release. Android installs an update only over an app signed
with the same key: **losing it means nobody can update in place**, so keep two backups
outside this laptop (for example a USB stick and a password manager).

```sh
# Creates .local/android/aurora-release.jks and its password file (both ignored by Git)
pw=$(openssl rand -base64 33 | tr -d '/+=\n' | cut -c1-32)
printf '%s' "$pw" > .local/android/aurora-release.password && chmod 600 .local/android/aurora-release.password
AURORA_RELEASE_PASSWORD="$pw" keytool -genkeypair -keystore .local/android/aurora-release.jks \
  -storetype PKCS12 -alias aurora -keyalg RSA -keysize 4096 -validity 10000 \
  -dname 'CN=Horizon Aurora' -storepass:env AURORA_RELEASE_PASSWORD -keypass:env AURORA_RELEASE_PASSWORD
```

Then give it to GitHub Actions (repository secrets):

```sh
base64 -w0 .local/android/aurora-release.jks | gh secret set AURORA_RELEASE_KEYSTORE_BASE64 -R IAmFuckingGenius/horizon-aurora
gh secret set AURORA_RELEASE_PASSWORD -R IAmFuckingGenius/horizon-aurora < .local/android/aurora-release.password
```

## Making a release

1. Set the new `versionName` in `packaging/android/build.gradle` (`0.7.7-aurora.2`, or
   `0.7.8-aurora.1` after moving to a newer Horizon Android).
2. Write the notes: `docs/releases/aurora-<version>.md` (what changed, which devices were
   tested, known limits).
3. Commit both to `main`, then tag that commit and push the tag:

   ```sh
   git tag v0.7.7-aurora.2 && git push aurora v0.7.7-aurora.2
   ```

4. The **Horizon Aurora release** workflow checks that the tag matches the version and the
   notes exist, builds the four public APKs from the tagged commit, signs them with the
   release key, audits them and opens a **draft** release with the APKs, `SHA256SUMS.txt`,
   `LICENSE.txt` and `THIRD_PARTY_NOTICES.md`.
5. Install the draft's arm64 APK on a phone and check it starts, downloads the resources
   and opens a world. Then press **Publish** on the draft.

A rerun replaces its own draft; a published release is never replaced. To fix a
published release, release a new version.

A local `python3 tools/android_build.py --public` build without `--release-key` is
signed with the development key: it is for testing and cannot update an installed release.
