#!/usr/bin/env python3
"""Run the Android launcher's engine and instance tests and the boot library's tests on the
host: no Android, no game, no player data. Covers checksum, ABI, ELF, size, id and
format checks, replacing an engine, and the boot library's one-start launch request."""
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def locked(lock, name, major):
    return next(p['version'] for p in lock['package']
                if p['name'] == name and p['version'].startswith(major))


def main():
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
