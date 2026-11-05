#!/usr/bin/env python3
"""Pack an Android engine: the game library built from this tree, as a `.hengine`.

An engine is the full game (`libhorizon_android.so`, with any native mods compiled in)
that Horizon Android's launcher installs into `files/engines/<id>/` and starts in place
of the APK's own engine, without reinstalling the app. The file is a ZIP:

    engine.json              format, id, name, version, commit, ABI, size, SHA-256, shell
    libhorizon_android.so    the stripped engine library

Usage:
    python3 tools/android_engine_pack.py --abi arm64-v8a --name "PvP build"
    python3 tools/android_engine_pack.py --abi x86_64 --skip-build --id test-1

The library is built with `tools/android_build.py --rust-only` (same profile and
features as the APK's) unless --skip-build is given. Output goes to
`.local/android/engines/<id>-<abi>.hengine`.
"""
import argparse
import datetime
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    'arm64-v8a': 'aarch64-linux-android',
    'armeabi-v7a': 'armv7-linux-androideabi',
    'x86_64': 'x86_64-linux-android',
    'x86': 'i686-linux-android',
}
# What the launcher accepts; bump when the engine.json contract changes.
FORMAT = 1
LIBRARY = 'libhorizon_android.so'
# ELF e_machine per ABI, checked again by the launcher on import.
MACHINES = {'arm64-v8a': 183, 'armeabi-v7a': 40, 'x86_64': 62, 'x86': 3}


def git(*args):
    try:
        return subprocess.check_output(['git', *args], cwd=ROOT, text=True,
                                       stderr=subprocess.DEVNULL).strip()
    except (OSError, subprocess.CalledProcessError):
        return ''


def app_version():
    """The Android app's version (versionName in packaging/android/build.gradle)."""
    text = (ROOT / 'packaging/android/build.gradle').read_text()
    match = re.search(r"versionName\s+'([^']+)'", text)
    return match.group(1) if match else '0'


def shell_level():
    """The shell level of this tree (horizon_boot::SHELL): the app an engine needs."""
    text = (ROOT / 'crates/horizon-boot/src/lib.rs').read_text()
    return int(re.search(r'pub const SHELL: u32 = (\d+);', text).group(1))


def ndk_strip():
    sdk = Path(os.environ.get('ANDROID_HOME', Path.home() / 'Android/Sdk'))
    for tool in sorted((sdk / 'ndk').glob('*/toolchains/llvm/prebuilt/*/bin/llvm-strip'), reverse=True):
        return tool
    return shutil.which('llvm-strip')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--abi', choices=TARGETS, default='arm64-v8a')
    parser.add_argument('--name', help='Name shown in the launcher (default: the branch and commit)')
    parser.add_argument('--id', help='Folder name on the phone: letters, digits, - _ . (default: from the commit)')
    parser.add_argument('--profile', choices=['fast', 'release', 'dev'], default='fast')
    parser.add_argument('--skip-build', action='store_true', help='Pack the library already built')
    parser.add_argument('--version', help='Version shown in the launcher (default: versionName in build.gradle)')
    parser.add_argument('--output', type=Path, help='Where to write the .hengine')
    args = parser.parse_args()

    commit = git('rev-parse', '--short=12', 'HEAD') or 'unknown'
    dirty = bool(git('status', '--porcelain', '--untracked-files=no'))
    branch = git('rev-parse', '--abbrev-ref', 'HEAD') or 'local'
    engine_id = args.id or f'{branch}-{commit[:8]}{"-dirty" if dirty else ""}'
    engine_id = re.sub(r'[^A-Za-z0-9._-]', '-', engine_id).lstrip('.')[:64]
    if not engine_id:
        parser.error('the id is empty')
    name = (args.name or f'{branch} {commit[:8]}').strip()[:48]

    if not args.skip_build:
        subprocess.check_call([sys.executable, ROOT / 'tools/android_build.py', '--abi', args.abi,
                               '--rust-only', '--profile', args.profile], cwd=ROOT)
    target_dir = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target')).resolve()
    built = target_dir / TARGETS[args.abi] / ('debug' if args.profile == 'dev' else args.profile) / LIBRARY
    if not built.is_file():
        sys.exit(f'No engine library at {built}: build it first (drop --skip-build).')

    with tempfile.TemporaryDirectory() as temp:
        library = Path(temp) / LIBRARY
        shutil.copyfile(built, library)
        strip = ndk_strip()
        if strip:
            subprocess.check_call([strip, '--strip-unneeded', library])
        data = library.read_bytes()
        machine = int.from_bytes(data[18:20], 'little')
        if data[:4] != b'\x7fELF' or machine != MACHINES[args.abi]:
            sys.exit(f'{built} is not an {args.abi} library')
        manifest = {
            'format': FORMAT,
            'id': engine_id,
            'name': name,
            'version': args.version or app_version(),
            'commit': commit,
            'dirty': dirty,
            'abi': args.abi,
            'built': datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
            'size': len(data),
            'sha256': hashlib.sha256(data).hexdigest(),
            'shell': shell_level(),
        }
        output = args.output or ROOT / '.local/android/engines' / f'{engine_id}-{args.abi}.hengine'
        output.parent.mkdir(parents=True, exist_ok=True)
        partial = output.with_suffix('.partial')
        with zipfile.ZipFile(partial, 'w') as archive:
            archive.writestr('engine.json', json.dumps(manifest, indent=2) + '\n',
                             compress_type=zipfile.ZIP_DEFLATED)
            archive.write(library, LIBRARY, compress_type=zipfile.ZIP_DEFLATED)
        partial.replace(output)
    print(f'Engine: {output} ({output.stat().st_size / 1e6:.1f} MB, id {engine_id})')


if __name__ == '__main__':
    main()
