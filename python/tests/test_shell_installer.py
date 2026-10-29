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
    """Stub curl and tar without touching the network."""
    stub_dir.mkdir(parents=True, exist_ok=True)
    curl = stub_dir / "curl"
    curl.write_text(
        "#!/bin/sh\n"
        'out=""; prev=""; effective="0"; url=""\n'
        'for a in "$@"; do\n'
        '  [ "$prev" = "-o" ] && out="$a"\n'
        '  [ "$prev" = "-w" ] && effective="1"\n'
        '  url="$a"; prev="$a"\n'
        "done\n"
        'echo "$@" >> "$LFTEST_LOG"\n'
        'if [ -n "$out" ] && [ "$out" != "/dev/null" ]; then\n'
        '  if [ "${url##*/}" = "SHA256SUMS" ]; then\n'
        "    digest=$(printf \"dummy\\n\" | shasum -a 256 | awk '{ print $1 }')\n"
        '    [ "${LFTEST_BAD_SUMS:-0}" = "1" ] && '
        'digest="0000000000000000000000000000000000000000000000000000000000000000"\n'
        "    for name in lf-aarch64-apple-darwin.tar.gz "
        "lf-x86_64-apple-darwin.tar.gz lf-x86_64-unknown-linux-gnu.tar.gz "
        "lf-aarch64-unknown-linux-gnu.tar.gz Loopflow.dmg install.sh; do\n"
        '      echo "$digest  $name" >> "$out"\n'
        "    done\n"
        "  else\n"
        '    echo dummy > "$out"\n'
        "  fi\n"
        "fi\n"
        'case "$url" in\n'
        '  */releases\\?per_page=*) printf \'%s\' '
        '\'[{"tag_name":"v9.10.0-rc.1","draft":false,"prerelease":true},\' '
        '\'{"tag_name":"v9.9.10","draft":false,"prerelease":false},\' '
        '\'{"tag_name":"v9.9.9","draft":false,"prerelease":false}]\' ;;\n'
        "esac\n"
        'if [ "$effective" = "1" ]; then\n'
        '  case "$url" in\n'
        '    */releases/latest) printf "%s" '
        '"https://github.com/loopflowstudio/loopflow/releases/'
        'tag/v9.9.9" ;;\n'
        '    *) printf "%s" "https://release-assets.githubusercontent.com/asset" ;;\n'
        "  esac\n"
        "fi\n"
        "exit ${LFTEST_CURL_RC:-0}\n"
    )
    tar = stub_dir / "tar"
    tar.write_text(
        "#!/bin/sh\n"
        'dir=""; prev=""\n'
        'for a in "$@"; do [ "$prev" = "-C" ] && dir="$a"; prev="$a"; done\n'
        'if [ -n "$dir" ]; then\n'
        "  cat > \"$dir/lf\" <<'LF'\n"
        "#!/bin/sh\n"
        'printf \'%s\\n\' "$*" >> "$LFTEST_PROMOTE_LOG"\n'
        'prev=""\n'
        'for arg in "$@"; do\n'
        '  case "$prev" in\n'
        '    --cli-target) cp "$0" "$arg" ;;\n'
        "  esac\n"
        '  prev="$arg"\n'
        "done\n"
        "exit 0\n"
        "LF\n"
        "fi\n"
        "exit 0\n"
    )
    hdiutil = stub_dir / "hdiutil"
    hdiutil.write_text(
        "#!/bin/sh\n"
        'if [ "$1" = "attach" ]; then\n'
        '  prev=""; mount=""\n'
        '  for arg in "$@"; do [ "$prev" = "-mountpoint" ] && mount="$arg"; prev="$arg"; done\n'
        '  mkdir -p "$mount/Loopflow.app"\n'
        "fi\n"
        "exit 0\n"
    )
    codesign = stub_dir / "codesign"
    codesign.write_text("#!/bin/sh\nexit 0\n")
    spctl = stub_dir / "spctl"
    spctl.write_text("#!/bin/sh\nexit 0\n")
    uname = stub_dir / "uname"
    uname.write_text(
        "#!/bin/sh\n"
        'case "$1" in\n'
        '  -s) [ -n "${LFTEST_UNAME_SYSTEM:-}" ] && '
        'echo "$LFTEST_UNAME_SYSTEM" || /usr/bin/uname -s ;;\n'
        '  -m) [ -n "${LFTEST_UNAME_ARCH:-}" ] && '
        'echo "$LFTEST_UNAME_ARCH" || /usr/bin/uname -m ;;\n'
        '  *) /usr/bin/uname "$@" ;;\n'
        "esac\n"
    )
    for f in (curl, tar, hdiutil, codesign, spctl, uname):
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
        "LF_INSTALL_CLI_ONLY": "1",
        "HOME": str(tmp_path / "home"),
    }


def test_installer_syntax_is_valid(installer: Path) -> None:
    """Malformed shell would ship a broken installer to every user."""
    result = subprocess.run(["sh", "-n", str(installer)], capture_output=True, text=True)
    assert result.returncode == 0, result.stderr


def test_removed_no_interactive_flag_fails_clearly(installer: Path, env: dict[str, str]) -> None:
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
    promote, schedule = (tmp_path / "promote.log").read_text().splitlines()
    assert promote.split() == [
        "install",
        "promote",
        "--cli-target",
        str(tmp_path / "dest/lf"),
        "--sync-skills",
    ]
    assert schedule == "install schedule"


def test_latest_release_is_pinned_before_the_archive_download(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    result = _run(installer, [], env)
    assert result.returncode == 0, result.stderr
    downloads = (tmp_path / "curl.log").read_text()
    assert "/releases/latest" in downloads
    assert "/releases/download/v9.9.9/SHA256SUMS" in downloads
    assert "/releases/download/v9.9.9/lf-" in downloads


def test_digest_mismatch_aborts_before_promotion(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    result = _run(installer, [], {**env, "LFTEST_BAD_SUMS": "1"})
    assert result.returncode != 0
    assert "Digest mismatch" in result.stderr
    assert not (tmp_path / "promote.log").exists()


def test_macos_release_promotes_the_verified_app_with_the_control_plane(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    applications = tmp_path / "Applications"
    applications.mkdir()
    app_env = {
        **env,
        "LF_INSTALL_CLI_ONLY": "0",
        "LF_APPLICATIONS_DIR": str(applications),
        "LFTEST_UNAME_SYSTEM": "Darwin",
        "LFTEST_UNAME_ARCH": "arm64",
    }

    result = _run(installer, [], app_env)

    assert result.returncode == 0, result.stderr
    args = (tmp_path / "promote.log").read_text().splitlines()[0].split()
    assert "--app-source" in args
    assert args[args.index("--app-target") + 1] == str(applications / "Loopflow.app")
    assert args[args.index("--legacy-app-target") + 1] == str(applications / "Concerto.app")


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
        "HOME": str(tmp_path / "home"),
    }
    result = _run(installer, [], env_extra)
    assert result.returncode != 0
    assert "Download failed" in result.stderr
    assert "No such file" not in (result.stdout + result.stderr)


def test_preview_channel_installs_the_newest_release_and_records_it(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    config = tmp_path / "home/.lf/config.yaml"
    config.parent.mkdir(parents=True)
    config.write_text("repo_root: /src")
    result = _run(installer, ["--channel", "preview"], env)
    assert result.returncode == 0, result.stderr
    assert "/releases/download/v9.9.10/lf-" in (tmp_path / "curl.log").read_text()
    assert config.read_text() == "repo_root: /src\nstream: preview\n"


def test_rerun_follows_the_recorded_channel(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    config = tmp_path / "home/.lf/config.yaml"
    config.parent.mkdir(parents=True)
    config.write_text("stream: preview\n")
    result = _run(installer, [], env)
    assert result.returncode == 0, result.stderr
    assert "/releases/download/v9.9.10/" in (tmp_path / "curl.log").read_text()
    result = _run(installer, ["--channel=stable"], env)
    assert result.returncode == 0, result.stderr
    assert config.read_text() == "stream: stable\n"


def test_dev_channel_points_at_lf_install(installer: Path, env: dict[str, str]) -> None:
    result = _run(installer, ["--channel", "dev"], env)
    assert result.returncode != 0
    assert "lf install --dev" in result.stderr


def test_channel_and_version_conflict(installer: Path, env: dict[str, str]) -> None:
    result = _run(installer, ["--channel", "preview", "--version", "0.9.9"], env)
    assert result.returncode != 0
    assert "not both" in result.stderr
