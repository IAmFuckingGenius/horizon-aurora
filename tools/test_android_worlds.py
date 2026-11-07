#!/usr/bin/env python3
"""Run the Android launcher's world ZIP export/import tests on the host: no Android,
no game, no player data. Covers zip-slip, symbolic links, a missing level.dat,
name collisions and the streamed export, and which of Horizon Android's folders a
move from it takes (horizon_folders.rs)."""
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    harness = ROOT / '.local/android/world-archive-tests'
    (harness / 'src').mkdir(parents=True, exist_ok=True)
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))
    # The zip the Android library itself links (crates/horizon-client/Cargo.toml).
    version = next(p['version'] for p in lock['package']
                   if p['name'] == 'zip' and p['version'].startswith('9.'))
    (harness / 'Cargo.toml').write_text(f'''[package]
name = "android-world-archive-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
zip = {{ version = "={version}", default-features = false, features = ["deflate"] }}
''', encoding='utf-8')
    android = ROOT / 'crates/horizon-client/src/android'
    source = (android / 'world_archive.rs').as_posix()
    folders = (android / 'horizon_folders.rs').as_posix()
    (harness / 'src/lib.rs').write_text(f'''#![allow(dead_code)]
#[path = "{source}"] mod world_archive;
#[path = "{folders}"] mod horizon_folders;
''', encoding='utf-8')
    subprocess.run(['cargo', 'test', '--manifest-path', str(harness / 'Cargo.toml')],
                   cwd=ROOT, check=True)


if __name__ == '__main__':
    main()
