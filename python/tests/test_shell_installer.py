"""Tests for the versioned release shell installer.

These tests exercise its argument parsing and download error handling with stub
curl/tar binaries so no network is touched.
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
RELEASE_INSTALLER = REPO_ROOT / "release" / "install.sh"


def _write_stubs(stub_dir: Path) -> None:
    """Stub curl (logs args, fakes the download) and tar (drops lf)."""
    stub_dir.mkdir(parents=True, exist_ok=True)
    curl = stub_dir / "curl"
    curl.write_text(
        "#!/bin/sh\n"
        'out=""; prev=""\n'
        'for a in "$@"; do [ "$prev" = "-o" ] && out="$a"; prev="$a"; done\n'
        'echo "$@" >> "$LFTEST_LOG"\n'
        '[ -n "$out" ] && echo dummy > "$out"\n'
        "exit ${LFTEST_CURL_RC:-0}\n"
    )
    tar = stub_dir / "tar"
    tar.write_text(
        "#!/bin/sh\n"
        'dir=""; prev=""\n'
        'for a in "$@"; do [ "$prev" = "-C" ] && dir="$a"; prev="$a"; done\n'
        'if [ -n "$dir" ]; then\n'
        '  cat > "$dir/lf" <<\'LF\'\n'
        "#!/bin/sh\n"
        "printf '%s\\n' \"$*\" >> \"$LFTEST_PROMOTE_LOG\"\n"
        "exit 0\n"
        "LF\n"
        "fi\n"
        "exit 0\n"
    )
    for f in (curl, tar):
        f.chmod(0o755)


def _run(script: Path, args: list[str], env_extra: dict[str, str]) -> subprocess.CompletedProcess:
    env = {**os.environ, **env_extra}
    return subprocess.run(
        ["sh", str(script), *args],
        capture_output=True,
        text=True,
        env=env,
    )


@pytest.fixture
def installer() -> Path:
    return RELEASE_INSTALLER


@pytest.fixture
def env(tmp_path: Path) -> dict[str, str]:
    stub_dir = tmp_path / "stub"
    _write_stubs(stub_dir)
    return {
        "PATH": f"{stub_dir}:{os.environ['PATH']}",
        "LF_INSTALL_DIR": str(tmp_path / "dest"),
        "LFTEST_LOG": str(tmp_path / "curl.log"),
        "LFTEST_PROMOTE_LOG": str(tmp_path / "promote.log"),
    }


def test_installer_syntax_is_valid(installer: Path) -> None:
    """Malformed shell would ship a broken installer to every user."""
    result = subprocess.run(["sh", "-n", str(installer)], capture_output=True, text=True)
    assert result.returncode == 0, result.stderr


def test_removed_no_interactive_flag_fails_clearly(
    installer: Path, env: dict[str, str]
) -> None:
    result = _run(installer, ["--no-interactive"], env)
    assert result.returncode != 0
    assert "Unknown option" in result.stderr


def test_positional_version_builds_versioned_url(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    result = _run(installer, ["0.9.9"], env)
    assert result.returncode == 0, result.stderr
    assert "download/v0.9.9/" in (tmp_path / "curl.log").read_text()


def test_explicit_version_builds_versioned_url(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    result = _run(installer, ["--version", "0.9.9"], env)
    assert result.returncode == 0, result.stderr
    assert "download/v0.9.9/" in (tmp_path / "curl.log").read_text()


def test_equals_version_builds_versioned_url(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    result = _run(installer, ["--version=0.9.9"], env)
    assert result.returncode == 0, result.stderr
    assert "download/v0.9.9/" in (tmp_path / "curl.log").read_text()


def test_downloaded_candidate_owns_activation(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    result = _run(installer, [], env)
    assert result.returncode == 0, result.stderr
    assert (tmp_path / "promote.log").read_text().strip() == (
        f"install promote --cli-target {tmp_path / 'dest/lf'}"
    )


def test_missing_version_value_fails_clearly(installer: Path, env: dict[str, str]) -> None:
    result = _run(installer, ["--version"], env)
    assert result.returncode != 0
    assert "--version requires a value" in result.stderr
    assert "Usage:" in result.stderr


def test_unknown_flag_fails_clearly(installer: Path, env: dict[str, str]) -> None:
    result = _run(installer, ["--bogus"], env)
    assert result.returncode != 0
    assert "Unknown option" in result.stderr
    assert "Usage:" in result.stderr


def test_download_failure_aborts_without_cryptic_error(installer: Path, tmp_path: Path) -> None:
    """A failed download must abort loudly, not leak a confusing `cp` error."""
    stub_dir = tmp_path / "stub"
    _write_stubs(stub_dir)
    env_extra = {
        "PATH": f"{stub_dir}:{os.environ['PATH']}",
        "LF_INSTALL_DIR": str(tmp_path / "dest"),
        "LFTEST_LOG": str(tmp_path / "curl.log"),
        "LFTEST_CURL_RC": "22",
    }
    result = _run(installer, [], env_extra)
    assert result.returncode != 0
    assert "Download failed" in result.stderr
    assert "No such file" not in (result.stdout + result.stderr)
