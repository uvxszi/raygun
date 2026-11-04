"""Keeps the Cargo target directory to what the next build uses.

Rust's incremental caches and Raygun's own crates are rebuilt under a new hash whenever their
settings change (another profile, target, feature set or `--config`), and Cargo never removes the
old ones. This removes, for Raygun's workspace crates only:

- incremental caches other than the newest per crate, profile and target, and any untouched for
  `--days`;
- compiled artifacts of the workspace crates other than the newest per crate, kind and profile.

Dependencies are left alone, so nothing slow is rebuilt; at worst one workspace crate's
incremental cache starts over. Run `python tools/trim_target.py` (dry run: `--dry-run`); `pnpm dev`
and `pnpm android:dev` run it first.
"""

import argparse
import re
import shutil
import sys
import time
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKSPACE = re.compile(r"^(lib)?(raygun[a-z_]*|tauri_plugin_raygun_audio)-([0-9a-f]{16})(\..+)?$")


def size(path: Path) -> int:
    if path.is_file():
        return path.stat().st_size
    return sum(item.stat().st_size for item in path.rglob("*") if item.is_file())


def remove(path: Path, dry_run: bool) -> int:
    freed = size(path)
    if not dry_run:
        if path.is_dir():
            shutil.rmtree(path, ignore_errors=True)
        else:
            path.unlink(missing_ok=True)
    return freed


def profiles(target: Path):
    """Every `<target>/<profile>` and `<target>/<triple>/<profile>` folder that holds builds."""
    for folder in target.rglob("deps"):
        if folder.parent.parent in (target, *target.iterdir()):
            yield folder.parent


def trim_incremental(profile: Path, days: float, dry_run: bool) -> int:
    folder = profile / "incremental"
    if not folder.is_dir():
        return 0
    freed = 0
    cutoff = time.time() - days * 86400
    newest: dict[str, Path] = {}
    for cache in folder.iterdir():
        crate = cache.name.rsplit("-", 1)[0]
        if not WORKSPACE.match(f"{crate.replace('-', '_')}-0000000000000000"):
            continue
        if cache.stat().st_mtime < cutoff:
            freed += remove(cache, dry_run)
            continue
        kept = newest.get(crate)
        if kept is None or cache.stat().st_mtime > kept.stat().st_mtime:
            if kept is not None:
                freed += remove(kept, dry_run)
            newest[crate] = cache
        else:
            freed += remove(cache, dry_run)
    return freed


def trim_deps(profile: Path, dry_run: bool) -> int:
    folder = profile / "deps"
    groups: dict[tuple[str, str], dict[str, list[Path]]] = defaultdict(lambda: defaultdict(list))
    for item in folder.iterdir():
        found = WORKSPACE.match(item.name)
        if not found:
            continue
        prefix, crate, hash_, suffix = found.groups()
        kind = f"{prefix or ''}{suffix or ''}"
        groups[(crate, kind)][hash_].append(item)
    freed = 0
    for builds in groups.values():
        if len(builds) < 2:
            continue
        latest = max(builds, key=lambda hash_: max(p.stat().st_mtime for p in builds[hash_]))
        for hash_, items in builds.items():
            if hash_ != latest:
                freed += sum(remove(item, dry_run) for item in items)
    return freed


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--target", type=Path, default=ROOT / "target")
    parser.add_argument("--days", type=float, default=14)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if not args.target.is_dir():
        return 0
    freed = 0
    for profile in profiles(args.target):
        freed += trim_incremental(profile, args.days, args.dry_run)
        freed += trim_deps(profile, args.dry_run)
    verb = "Would free" if args.dry_run else "Freed"
    print(f"{verb} {freed / 2**30:.1f} GB in {args.target}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
