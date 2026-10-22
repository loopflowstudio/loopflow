from pathlib import Path

import pytest
import tomllib

from scripts.rust_profile_cache_key import profile_cache_key


def test_profile_changes_invalidate_but_version_and_formatting_do_not(tmp_path: Path):
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        '[workspace.package]\nversion = "1.0.0"\n'
        "[profile.dev.package.libsqlite3-sys]\nopt-level = 2\n"
    )
    initial = profile_cache_key(manifest)
    manifest.write_text(
        "# A new release does not change the build profile.\n"
        "[profile.dev.package.libsqlite3-sys]\nopt-level=2\n"
        '[workspace.package]\nversion="1.1.0"\n'
    )
    assert profile_cache_key(manifest) == initial
    manifest.write_text(manifest.read_text().replace("opt-level=2", "opt-level=3"))
    assert profile_cache_key(manifest) != initial


def test_malformed_manifest_cannot_reuse_a_profile_key(tmp_path: Path):
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text("[profile.dev\n")
    with pytest.raises(tomllib.TOMLDecodeError):
        profile_cache_key(manifest)
