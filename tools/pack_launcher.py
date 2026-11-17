#!/usr/bin/env python3
"""Packs a launcher release zip deterministically: the executable at the root, then LICENSE.

usage: pack_launcher.py EXE OUT.zip [LICENSE]

The same inputs give the same bytes (fixed times, fixed order, fixed modes, no extra fields), so a rebuild of an
unchanged binary has the same SHA-256.
"""
import os
import sys
import zipfile

FIXED_TIME = (1980, 1, 1, 0, 0, 0)


def add(archive, path, name, mode):
    info = zipfile.ZipInfo(name, FIXED_TIME)
    info.compress_type = zipfile.ZIP_DEFLATED
    info.create_system = 3
    info.external_attr = (0o100000 | mode) << 16
    with open(path, "rb") as source:
        archive.writestr(info, source.read(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)


def main():
    if len(sys.argv) not in (3, 4):
        sys.exit(__doc__)
    exe, out = sys.argv[1], sys.argv[2]
    name = "castanea-launcher.exe" if exe.endswith(".exe") else "castanea-launcher"
    if os.path.exists(out):
        os.remove(out)
    with zipfile.ZipFile(out, "w") as archive:
        add(archive, exe, name, 0o755)
        if len(sys.argv) == 4:
            add(archive, sys.argv[3], "LICENSE", 0o644)


main()
