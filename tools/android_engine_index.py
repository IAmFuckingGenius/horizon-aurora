#!/usr/bin/env python3
"""Write the list of engines a release offers: `engines.json`, published beside the
`.hengine` files it names, which the Android launcher reads (Settings › Engines).

    python3 tools/android_engine_index.py --base-url URL --output dist/engines.json dist/*.hengine

Each entry is the engine's own `engine.json` (id, name, version, commit, ABI, shell,
build time) with the file's address (`--base-url` + its name), size and SHA-256, which
the launcher checks the download against.
"""
import argparse
import hashlib
import json
import sys
import zipfile
from pathlib import Path

# The list format the launcher reads (`engines::FORMAT`).
FORMAT = 1
FIELDS = ('id', 'name', 'version', 'commit', 'abi', 'shell', 'built')


def entry(path, base_url):
    with zipfile.ZipFile(path) as archive:
        manifest = json.loads(archive.read('engine.json'))
    if manifest.get('format') != FORMAT:
        sys.exit(f'{path}: engine.json format {manifest.get("format")}, expected {FORMAT}')
    data = path.read_bytes()
    return {**{key: manifest.get(key, 0 if key == 'shell' else '') for key in FIELDS},
            'url': base_url + path.name, 'size': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--base-url', required=True, help='Where the files are published, ending in /')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('engines', type=Path, nargs='+')
    args = parser.parse_args()
    if not args.base_url.startswith('https://') or not args.base_url.endswith('/'):
        parser.error('--base-url must be an https:// address ending in /')
    engines = [entry(path, args.base_url) for path in sorted(args.engines)]
    args.output.write_text(json.dumps({'format': FORMAT, 'engines': engines}, indent=2) + '\n', encoding='utf-8')
    print(f'{args.output}: {len(engines)} engine(s)')


if __name__ == '__main__':
    main()
