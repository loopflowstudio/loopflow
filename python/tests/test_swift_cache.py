import json
import os
import subprocess
from pathlib import Path

import pytest

from scripts.swift_cache import INDEX, INDEXES, cache_key, restore_source_times, save_source_times


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    subprocess.run(["git", "init", "-q", str(tmp_path)], check=True)
    (tmp_path / "swift").mkdir()
    return tmp_path


def _track(repo: Path, name: str, contents: str) -> Path:
    path = repo / "swift" / name
    path.write_text(contents)
    subprocess.run(["git", "add", "--", str(path)], cwd=repo, check=True)
    return path


def test_restore_unchanged_inputs_but_keep_changed_content_and_checkout_time(repo: Path) -> None:
    unchanged = _track(repo, "unchanged.swift", "first")
    changed = _track(repo, "changed.swift", "first")
    original = unchanged.stat().st_mtime_ns
    assert save_source_times(repo) == 2
    checkout = original + 1_000_000_000
    changed.write_text("other")  # Same byte count still needs recompilation.
    for path in (unchanged, changed):
        os.utime(path, ns=(checkout, checkout))

    assert restore_source_times(repo) == 1

    assert unchanged.stat().st_mtime_ns == original
    assert unchanged.stat().st_atime_ns == checkout
    assert changed.stat().st_mtime_ns == checkout
    assert changed.read_text() == "other"


def test_new_deleted_and_symlink_inputs_never_receive_cached_times(repo: Path) -> None:
    deleted = _track(repo, "deleted.swift", "deleted")
    link = repo / "swift/link.swift"
    link.symlink_to(deleted)
    subprocess.run(["git", "add", "--", str(link)], cwd=repo, check=True)
    assert save_source_times(repo) == 1
    deleted.unlink()
    added = _track(repo, "added.swift", "new")
    link.unlink()
    link.symlink_to(added)
    before = added.stat().st_mtime_ns

    assert restore_source_times(repo) == 0
    assert added.stat().st_mtime_ns == before
    assert not deleted.exists()
    assert link.is_symlink()


@pytest.mark.parametrize("metadata", [None, "broken json", "[]", '{"swift/a.swift": {}}'])
def test_missing_or_invalid_index_retains_checkout_timestamps(
    repo: Path, metadata: str | None
) -> None:
    source = _track(repo, "a.swift", "content")
    before = source.stat().st_mtime_ns
    if metadata is not None:
        index = repo / INDEX
        index.parent.mkdir(parents=True)
        index.write_text(metadata)
    assert restore_source_times(repo) == 0
    assert source.stat().st_mtime_ns == before


def test_cached_paths_cannot_modify_untracked_files(repo: Path) -> None:
    source = _track(repo, "a.swift", "content")
    save_source_times(repo)
    index = repo / INDEX
    records = json.loads(index.read_text())
    outside = repo / "outside.swift"
    outside.write_text("content")
    before = outside.stat().st_mtime_ns
    records["../outside.swift"] = {**records["swift/a.swift"], "mtime_ns": 1}
    records[str(outside)] = records["../outside.swift"]
    index.write_text(json.dumps(records))

    assert restore_source_times(repo) == 1
    assert outside.stat().st_mtime_ns == before
    assert source.read_text() == "content"


def test_cache_key_tracks_contents_and_toolchain_but_not_timestamps(repo: Path) -> None:
    source = _track(repo, "a.swift", "first")
    key, prefix = cache_key(repo, "compiler and SDK")
    os.utime(source, None)
    assert cache_key(repo, "compiler and SDK") == (key, prefix)
    source.write_text("other")
    changed, changed_prefix = cache_key(repo, "compiler and SDK")
    assert changed != key
    assert changed_prefix == prefix
    upgraded, upgraded_prefix = cache_key(repo, "new compiler and SDK")
    assert upgraded != changed
    assert upgraded_prefix != prefix
    save_source_times(repo)
    source.chmod(0o755)
    assert cache_key(repo, "compiler and SDK")[0] != changed
    assert restore_source_times(repo) == 0
    source.unlink()
    assert cache_key(repo, "compiler and SDK")[0] != changed


def test_xcode_and_swiftpm_keep_separate_build_times(repo: Path) -> None:
    source = _track(repo, "a.swift", "first")
    original = source.stat().st_mtime_ns
    save_source_times(repo, "xcode")
    assert (repo / INDEXES["xcode"]).is_file()
    assert not (repo / INDEX).exists()
    checkout = original + 1_000_000_000
    os.utime(source, ns=(checkout, checkout))
    save_source_times(repo)
    assert restore_source_times(repo, "xcode") == 1
    assert source.stat().st_mtime_ns == original
    assert restore_source_times(repo) == 1
    assert source.stat().st_mtime_ns == checkout
    source.write_text("other")
    changed = source.stat().st_mtime_ns
    assert restore_source_times(repo, "xcode") == 0
    assert source.stat().st_mtime_ns == changed


def test_xcode_key_separates_build_system_and_entitlement_changes(repo: Path) -> None:
    source = _track(repo, "Loopflow.entitlements", "<dict/>")
    key, prefix = cache_key(repo, "compiler and SDK", "xcode")
    assert key != cache_key(repo, "compiler and SDK")[0]
    source.write_text("<dict><key>com.apple.security.network.client</key><true/></dict>")
    changed, changed_prefix = cache_key(repo, "compiler and SDK", "xcode")
    assert changed != key
    assert changed_prefix == prefix
    assert cache_key(repo, "new Xcode or generator", "xcode")[0] != changed
