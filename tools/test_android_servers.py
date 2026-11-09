#!/usr/bin/env python3
"""Check how Aurora picks the release to join a server with (the older-servers adapter's
`adapter/negotiate.rs`): version names read as release ceilings, the newest protocol within one,
and the search for the newest protocol a server echoes. The module has no dependencies, so its
tests build with `rustc --test` alone, without the protocol crate.
"""
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'crates/horizon-protocol/src/adapter/negotiate.rs'


def main():
    with tempfile.TemporaryDirectory() as folder:
        binary = Path(folder) / 'negotiate-tests'
        subprocess.run(['rustc', '--edition', '2024', '--test', '--crate-name', 'negotiate',
                        str(SOURCE), '-o', str(binary)], check=True)
        subprocess.run([str(binary), '--quiet'], check=True)


if __name__ == '__main__':
    main()
