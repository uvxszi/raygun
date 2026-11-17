"""Builds the Smart Chips' packages and the catalogue that pins them.

Each folder in `chips/` (except `packages/`) is a chip: its `chip.json` (id, name, description,
kind, version, and optionally `from`, a folder of the app whose files the package carries) and any
other files beside it. The package is `chips/packages/<id>-<version>.tar.gz`, built the same way
every time (sorted names, no times or owners, LF line endings for text) so its SHA-256 only changes
when its files do. The catalogue, `crates/raygun-app-base/src/chips/catalog.json`, gets each
package's address on GitHub raw, size and hash.

    python tools/build_chips.py            # rebuild packages and the catalogue
    python tools/build_chips.py --check    # fail when either is out of date
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import sys
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHIPS = ROOT / "chips"
PACKAGES = CHIPS / "packages"
CATALOG = ROOT / "crates" / "raygun-app-base" / "src" / "chips" / "catalog.json"
RAW = "https://raw.githubusercontent.com/uvxszi/raygun/main/chips/packages/"
TEXT = {".json", ".glsl", ".txt", ".md", ".js"}


def files_of(folder: Path, manifest: dict) -> list[tuple[str, bytes]]:
    """The package's files as (name, bytes): the chip's own folder, then its `from` folder."""
    found: dict[str, bytes] = {}
    sources = [folder]
    if manifest.get("from"):
        sources.append(ROOT / manifest["from"])
    for source in sources:
        for path in sorted(source.rglob("*")):
            if not path.is_file():
                continue
            name = path.relative_to(source).as_posix()
            data = path.read_bytes()
            if path.suffix in TEXT:
                data = data.replace(b"\r\n", b"\n")
            found[name] = data
    return sorted(found.items())


def package(files: list[tuple[str, bytes]]) -> bytes:
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.USTAR_FORMAT) as tar:
        for name, data in files:
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mtime = 0
            info.mode = 0o644
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            tar.addfile(info, io.BytesIO(data))
    packed = io.BytesIO()
    with gzip.GzipFile(filename="", mode="wb", fileobj=packed, mtime=0, compresslevel=9) as out:
        out.write(raw.getvalue())
    return packed.getvalue()


def build() -> tuple[dict[Path, bytes], str]:
    outputs: dict[Path, bytes] = {}
    catalog = []
    for folder in sorted(CHIPS.iterdir()):
        if not folder.is_dir() or folder == PACKAGES:
            continue
        manifest = json.loads((folder / "chip.json").read_text(encoding="utf-8"))
        data = package(files_of(folder, manifest))
        name = f"{manifest['id']}-{manifest['version']}.tar.gz"
        outputs[PACKAGES / name] = data
        catalog.append(
            {
                "id": manifest["id"],
                "name": manifest["name"],
                "description": manifest["description"],
                "kind": manifest["kind"],
                "version": manifest["version"],
                "package": {
                    "url": RAW + name,
                    "sha256": hashlib.sha256(data).hexdigest(),
                    "size": len(data),
                },
            }
        )
    return outputs, json.dumps(catalog, indent=2, ensure_ascii=False) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="fail when something is out of date")
    check = parser.parse_args().check
    outputs, catalog = build()
    stale = [path for path, data in outputs.items() if not path.exists() or path.read_bytes() != data]
    kept = {path.name for path in outputs}
    leftover = [path for path in PACKAGES.glob("*.tar.gz") if path.name not in kept]
    catalog_stale = not CATALOG.exists() or CATALOG.read_text(encoding="utf-8") != catalog
    if check:
        for path in stale + leftover:
            print(f"out of date: {path.relative_to(ROOT)}")
        if catalog_stale:
            print(f"out of date: {CATALOG.relative_to(ROOT)}")
        return 1 if stale or leftover or catalog_stale else 0
    PACKAGES.mkdir(exist_ok=True)
    for path in stale:
        path.write_bytes(outputs[path])
        print(f"wrote {path.relative_to(ROOT)}")
    for path in leftover:
        path.unlink()
        print(f"removed {path.relative_to(ROOT)}")
    if catalog_stale:
        CATALOG.write_text(catalog, encoding="utf-8", newline="\n")
        print(f"wrote {CATALOG.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
