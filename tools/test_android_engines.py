#!/usr/bin/env python3
"""Run the Android launcher's engine and instance tests and the boot library's tests on the
host: no Android, no game, no player data. Covers checksum, ABI, ELF, size, id, format and
shell checks, replacing an engine, the engine lists, and the boot library's one-start
launch request. Also checks that the APK's Java side has not changed since its shell level
was recorded (`--record-shell` records it, after a bump of horizon_boot::SHELL if engines
built from this tree need the change)."""
from pathlib import Path
import hashlib
import json
import re
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def locked(lock, name, major):
    return next(p['version'] for p in lock['package']
                if p['name'] == name and p['version'].startswith(major))


SHELL_RECORD = ROOT / 'packaging/android/shell.json'


def shell_level():
    text = (ROOT / 'crates/horizon-boot/src/lib.rs').read_text(encoding='utf-8')
    return int(re.search(r'pub const SHELL: u32 = (\d+);', text).group(1))


def shell_fingerprint():
    """What an engine relies on in the APK besides the boot library: the Java side and
    the manifest."""
    android = ROOT / 'packaging/android'
    files = sorted([android / 'AndroidManifest.xml', *(p for p in (android / 'java').rglob('*') if p.is_file())])
    digest = hashlib.sha256()
    for path in files:
        digest.update(path.relative_to(ROOT).as_posix().encode() + b'\0' + path.read_bytes() + b'\0')
    return digest.hexdigest()


def check_shell(record):
    level, fingerprint = shell_level(), shell_fingerprint()
    if record:
        SHELL_RECORD.write_text(json.dumps({'shell': level, 'sha256': fingerprint}, indent=2) + '\n', encoding='utf-8')
        print(f'Recorded shell level {level}.')
        return
    recorded = json.loads(SHELL_RECORD.read_text(encoding='utf-8'))
    if recorded != {'shell': level, 'sha256': fingerprint}:
        sys.exit('The APK\'s Java side or manifest changed since shell level '
                 f'{recorded["shell"]} was recorded. If engines built from this tree need the change, '
                 'bump SHELL in crates/horizon-boot/src/lib.rs; then run '
                 'python3 tools/test_android_engines.py --record-shell.')


def main():
    check_shell('--record-shell' in sys.argv[1:])
    harness = ROOT / '.local/android/engine-tests'
    (harness / 'src').mkdir(parents=True, exist_ok=True)
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))
    boot = (ROOT / 'crates/horizon-boot').as_posix()
    (harness / 'Cargo.toml').write_text(f'''[package]
name = "android-engine-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
horizon-boot = {{ path = "{boot}" }}
zip = {{ version = "={locked(lock, 'zip', '2.')}", default-features = false, features = ["deflate"] }}
sha2 = "={locked(lock, 'sha2', '0.10')}"
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
''', encoding='utf-8')
    source = (ROOT / 'crates/horizon-client/src/android/engines.rs').as_posix()
    instances = (ROOT / 'crates/horizon-client/src/android/instances.rs').as_posix()
    (harness / 'src/lib.rs').write_text(f'''#![allow(dead_code)]
#[path = "{source}"] mod engines;
#[path = "{instances}"] mod instances;
''', encoding='utf-8')
    subprocess.run(['cargo', 'test', '--manifest-path', str(harness / 'Cargo.toml')],
                   cwd=ROOT, check=True)
    subprocess.run(['cargo', 'test', '-p', 'horizon-boot'], cwd=ROOT, check=True)


if __name__ == '__main__':
    main()
