"""Preserve unchanged Swift input timestamps with the CI build cache."""

import argparse
import hashlib
import json
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INDEX = Path("swift/.build/source-times.json")
INDEXES = {
    "swiftpm": INDEX,
    "xcode": Path("swift/.build/xcode-derived-data/source-times.json"),
}


def _tracked_sources(repo: Path) -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z", "--", "swift"], cwd=repo, capture_output=True, check=True
    )
    return sorted(repo / os.fsdecode(name) for name in result.stdout.split(b"\0") if name)


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cache_key(repo: Path, toolchain: str, build_system: str = "swiftpm") -> tuple[str, str]:
    toolchain_hash = hashlib.sha256(toolchain.encode()).hexdigest()
    sources = hashlib.sha256()
    for path in _tracked_sources(repo):
        sources.update(os.fsencode(path.relative_to(repo)))
        sources.update(b"\0")
        if path.is_symlink():
            value = "symlink:" + os.readlink(path)
        elif path.is_file():
            value = f"{path.stat().st_mode}:{_digest(path)}"
        else:
            value = "missing"
        sources.update(value.encode())
        sources.update(b"\0")
    prefix = f"{build_system}-source-times-{toolchain_hash}-"
    return prefix + sources.hexdigest(), prefix


def save_source_times(repo: Path, build_system: str = "swiftpm") -> int:
    records = {
        str(path.relative_to(repo)): {
            "sha256": _digest(path),
            "mtime_ns": path.stat().st_mtime_ns,
            "mode": path.stat().st_mode,
        }
        for path in _tracked_sources(repo)
        if path.is_file() and not path.is_symlink()
    }
    index = repo / INDEXES[build_system]
    index.parent.mkdir(parents=True, exist_ok=True)
    index.write_text(json.dumps(records, sort_keys=True) + "\n")
    return len(records)


def restore_source_times(repo: Path, build_system: str = "swiftpm") -> int:
    try:
        records = json.loads((repo / INDEXES[build_system]).read_text())
    except (OSError, ValueError):
        print("No readable Swift source timestamp index; keeping checkout timestamps")
        return 0
    if not isinstance(records, dict):
        print("Invalid Swift source timestamp index; keeping checkout timestamps")
        return 0
    restored = 0
    for path in _tracked_sources(repo):
        if not path.is_file() or path.is_symlink():
            continue
        record = records.get(str(path.relative_to(repo)))
        if not isinstance(record, dict):
            continue
        mtime = record.get("mtime_ns")
        if type(mtime) is not int or not 0 <= mtime < 2**63:
            continue
        metadata = path.stat()
        if record.get("mode") != metadata.st_mode or record.get("sha256") != _digest(path):
            continue
        os.utime(path, ns=(metadata.st_atime_ns, mtime), follow_symlinks=False)
        restored += 1
    return restored


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("key", "save", "restore"))
    parser.add_argument("--build-system", choices=INDEXES, default="swiftpm")
    args = parser.parse_args()
    if args.action == "key":
        commands = [["swift", "--version"], ["xcrun", "--show-sdk-version"]]
        if args.build_system == "xcode":
            commands.extend([["xcodebuild", "-version"], ["xcodegen", "--version"]])
        identity = "\n".join(
            subprocess.run(command, capture_output=True, text=True, check=True).stdout
            for command in commands
        )
        key, prefix = cache_key(ROOT, identity, args.build_system)
        print(f"key={key}")
        print(f"restore-prefix={prefix}")
    elif args.action == "save":
        count = save_source_times(ROOT, args.build_system)
        print(f"Saved Swift source timestamps for {count} files")
    else:
        count = restore_source_times(ROOT, args.build_system)
        print(f"Restored Swift source timestamps for {count} unchanged files")


if __name__ == "__main__":
    main()
