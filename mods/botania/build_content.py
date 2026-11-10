#!/usr/bin/env python3
"""Build `dist/botania.hmod`, Botania's blocks, items, recipes and assets as a Horizon content mod.

    python3 mods/botania/build_content.py

It downloads Botania 1.20.1-456 for Forge from Modrinth (checked against its SHA-1), carries it with
`horizon-port-pack check --old` (its data under 1.21's folder names, vanilla recipe results in 26.3's
form, the Forge biome modifiers that place the mystical flowers), lays `content/` over the result
(block shapes and properties, flower features, vanilla models for what Botania draws in code) and packs
it with `cargo horizon-mod build`. Run from the repository root; it builds the two tools if needed.

    python3 mods/botania/build_content.py --overlay

lays `content/` over the committed `dist/botania.hmod` instead, without the jar or the tools: for a
change to `content/` alone, or to bring a rebuilt archive (a merge) up to `content/` again.

Botania is by Vazkii, under the Botania License (`LICENSE` here): its code and assets may be used and
shared with credit, in an open source project, free of charge. This is a modified port.
"""
from pathlib import Path
import hashlib
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
JAR_URL = 'https://cdn.modrinth.com/data/pfjLUfGv/versions/npY4sRWF/Botania-1.20.1-456-FORGE.jar'
JAR_SHA1 = '17e0cab4160e86f434cd137a60faeb2594ae4e15'


def run(*args, cwd=ROOT):
    subprocess.run([str(a) for a in args], cwd=cwd, check=True)


def main():
    work = Path(tempfile.mkdtemp(prefix='botania-content-'))
    jar = work / 'Botania-1.20.1-456-FORGE.jar'
    with urllib.request.urlopen(JAR_URL, timeout=120) as response:
        jar.write_bytes(response.read())
    if hashlib.sha1(jar.read_bytes()).hexdigest() != JAR_SHA1:
        sys.exit(f'{JAR_URL}: not the jar this content was made from (SHA-1 differs)')
    run('cargo', 'build', '-q', '-p', 'horizon-port-pack', '-p', 'cargo-horizon-mod')
    target = ROOT / 'target/debug'
    run(target / 'horizon-port-pack', 'check', jar, work / 'check', '--old')
    carry = work / 'check/carry'
    for source in sorted((HERE / 'content').rglob('*')):
        if source.is_file():
            destination = carry / source.relative_to(HERE / 'content')
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
    run(target / 'cargo-horizon-mod', 'horizon-mod', 'build', cwd=carry)
    out = HERE / 'dist/botania.hmod'
    out.parent.mkdir(exist_ok=True)
    shutil.copyfile(carry / 'target/horizon-mod/botania.hmod', out)
    print(f'{out}: {out.stat().st_size / 1e6:.1f} MB, SHA-256 {hashlib.sha256(out.read_bytes()).hexdigest()}')
    shutil.rmtree(work)


def overlay():
    """`content/`'s files replace or join the archive's of the same paths; the rest stays as it is."""
    out = HERE / 'dist/botania.hmod'
    files = {
        source.relative_to(HERE / 'content').as_posix(): source.read_bytes()
        for source in sorted((HERE / 'content').rglob('*'))
        if source.is_file()
    }
    staged = out.with_suffix('.hmod.tmp')
    with zipfile.ZipFile(out) as old, zipfile.ZipFile(staged, 'w') as new:
        for info in old.infolist():
            data = files.pop(info.filename, None)
            new.writestr(info, old.read(info) if data is None else data, compress_type=info.compress_type)
        for name, data in files.items():
            new.writestr(zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0)), data, compress_type=zipfile.ZIP_DEFLATED)
    staged.replace(out)
    print(f'{out}: {out.stat().st_size / 1e6:.1f} MB, SHA-256 {hashlib.sha256(out.read_bytes()).hexdigest()}')


if __name__ == '__main__':
    overlay() if sys.argv[1:] == ['--overlay'] else main()
