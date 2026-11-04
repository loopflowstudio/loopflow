"""Tests for the versioned release shell installer.

Exercise download failures, destination selection and recovery with stub tools;
no network, installed apps or production receipts are touched.
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
        r"""#!/bin/sh
out=""; prev=""; effective="0"; url=""
for a in "$@"; do
  [ "$prev" = "-o" ] && out="$a"
  [ "$prev" = "-w" ] && effective="1"
  url="$a"; prev="$a"
done
echo "$@" >> "$LFTEST_LOG"
if [ -n "$out" ] && [ "$out" != "/dev/null" ]; then
  if [ "${url##*/}" = "SHA256SUMS" ]; then
    digest=$(printf "dummy\n" | shasum -a 256 | awk '{ print $1 }')
    [ "${LFTEST_BAD_SUMS:-0}" = "1" ] && digest="wrong-digest"
    for name in lf-aarch64-apple-darwin.tar.gz lf-x86_64-apple-darwin.tar.gz \
      lf-x86_64-unknown-linux-gnu.tar.gz lf-aarch64-unknown-linux-gnu.tar.gz \
      Loopflow.dmg install.sh; do
      echo "$digest  $name" >> "$out"
    done
  else
    echo dummy > "$out"
  fi
fi
case "$url" in
  */releases\?per_page=*)
    printf '%s' \
      '[{"tag_name":"v9.10.0-rc.1","draft":false,"prerelease":true},' \
      '{"tag_name":"v9.9.10","draft":false,"prerelease":false},' \
      '{"tag_name":"v9.9.9","draft":false,"prerelease":false}]' ;;
esac
if [ "$effective" = "1" ]; then
  case "$url" in
    */releases/latest)
      printf "%s" "https://github.com/loopflowstudio/loopflow/releases/tag/v9.9.9" ;;
    *) printf "%s" "https://release-assets.githubusercontent.com/asset" ;;
  esac
fi
exit ${LFTEST_CURL_RC:-0}
"""
    )
    tar = stub_dir / "tar"
    tar.write_text(
        r"""#!/bin/sh
dir=""; prev=""
for a in "$@"; do [ "$prev" = "-C" ] && dir="$a"; prev="$a"; done
if [ -n "$dir" ]; then
  cat > "$dir/lf" <<'LF'
#!/bin/sh
if [ "$*" = "install recover-switch --help" ]; then
  if [ "${LFTEST_OLDER_CANDIDATE:-0}" = "1" ]; then
    echo "Usage: lf install recover-switch --switch <SWITCH>"
  else
    echo "Usage: lf install recover-switch [OPTIONS]"
  fi
  exit 0
fi
if [ "$*" = "install recover-switch" ]; then
  [ "${LFTEST_OLDER_CANDIDATE:-0}" != "1" ] || exit 2
  [ -z "${LFTEST_RECOVER_APP:-}" ] ||
    ln -sf "$LFTEST_RECOVER_APP/Contents/MacOS/lf" "$LF_INSTALL_DIR/lf"
  exit "${LFTEST_RECOVER_RC:-0}"
fi
printf '%s\n' "$*" >> "$LFTEST_PROMOTE_LOG"
[ "$*" != "install schedule" ] || exit 0
[ "${LFTEST_PREFLIGHT_RC:-0}" = "0" ] || exit "$LFTEST_PREFLIGHT_RC"
prev=""; cli=""; app=""
for arg in "$@"; do
  case "$prev" in
    --cli-target) cli="$arg" ;;
    --app-target) app="$arg" ;;
  esac
  prev="$arg"
done
if [ -n "$app" ]; then
  mkdir -p "$app/Contents/MacOS"
  cp "$0" "$app/Contents/MacOS/lf"
  chmod +x "$app/Contents/MacOS/lf"
  ln -sf "$app/Contents/MacOS/lf" "$cli"
else
  cp "$0" "$cli"
fi
exit 0
LF
fi
exit 0
"""
    )
    hdiutil = stub_dir / "hdiutil"
    hdiutil.write_text(
        r"""#!/bin/sh
if [ "$1" = "attach" ]; then
  prev=""; mount=""
  for arg in "$@"; do [ "$prev" = "-mountpoint" ] && mount="$arg"; prev="$arg"; done
  mkdir -p "$mount/Loopflow.app"
fi
exit 0
"""
    )
    codesign = stub_dir / "codesign"
    codesign.write_text("#!/bin/sh\nexit 0\n")
    spctl = stub_dir / "spctl"
    spctl.write_text("#!/bin/sh\nexit 0\n")
    uname = stub_dir / "uname"
    uname.write_text(
        r"""#!/bin/sh
case "$1" in
  -s) [ -n "${LFTEST_UNAME_SYSTEM:-}" ] && echo "$LFTEST_UNAME_SYSTEM" || /usr/bin/uname -s ;;
  -m) [ -n "${LFTEST_UNAME_ARCH:-}" ] && echo "$LFTEST_UNAME_ARCH" || /usr/bin/uname -m ;;
  *) /usr/bin/uname "$@" ;;
esac
"""
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
        "LF_APPLICATIONS_DIR": str(tmp_path / "Applications"),
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


@pytest.mark.parametrize("args", [["0.9.9"], ["--version", "0.9.9"], ["--version=0.9.9"]])
def test_version_builds_versioned_url(
    installer: Path, env: dict[str, str], tmp_path: Path, args: list[str]
) -> None:
    result = _run(installer, args, env)
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


def test_download_failure_aborts_without_cryptic_error(
    installer: Path, env: dict[str, str]
) -> None:
    """A failed download must abort loudly, not leak a confusing `cp` error."""
    result = _run(installer, [], {**env, "LFTEST_CURL_RC": "22"})
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


@pytest.mark.parametrize("cli_only", ["environment", "flag"])
def test_cli_only_keeps_an_existing_app_in_the_update(
    installer: Path, env: dict[str, str], tmp_path: Path, cli_only: str
) -> None:
    applications = tmp_path / "Applications"
    (applications / "Loopflow.app").mkdir(parents=True)
    app_env = {
        **env,
        "LF_INSTALL_CLI_ONLY": "1" if cli_only == "environment" else "0",
        "LFTEST_UNAME_SYSTEM": "Darwin",
        "LFTEST_UNAME_ARCH": "arm64",
    }
    result = _run(installer, ["--cli-only"] if cli_only == "flag" else [], app_env)
    assert result.returncode == 0, result.stderr
    args = (tmp_path / "promote.log").read_text().splitlines()[0].split()
    assert args[args.index("--app-target") + 1] == str(applications / "Loopflow.app")
    assert "/Loopflow.dmg" in (tmp_path / "curl.log").read_text()


@pytest.mark.parametrize("cli_only", ["0", "1"])
@pytest.mark.parametrize("explicit", [False, True])
def test_custom_app_destination_survives_updates(
    installer: Path, env: dict[str, str], tmp_path: Path, cli_only: str, explicit: bool
) -> None:
    app = tmp_path / "Custom Tools/Loopflow.app"
    helper = app / "Contents/MacOS/lf"
    helper.parent.mkdir(parents=True)
    helper.write_text("old helper")
    public = tmp_path / "dest/lf"
    public.parent.mkdir()
    # Follow a relative link chain, including directories with spaces.
    public.symlink_to("next")
    (public.parent / "next").symlink_to("../Custom Tools/Loopflow.app/Contents/MacOS/lf")
    destination = tmp_path / "Other Tools" if explicit else app.parent
    destination.mkdir(exist_ok=True)
    result = _run(
        installer,
        [],
        {
            **env,
            "LF_INSTALL_CLI_ONLY": cli_only,
            "LF_APPLICATIONS_DIR": str(destination) if explicit else "",
            "LFTEST_UNAME_SYSTEM": "Darwin",
            "LFTEST_UNAME_ARCH": "arm64",
        },
    )
    assert result.returncode == 0, result.stderr
    assert public.resolve() == destination / "Loopflow.app/Contents/MacOS/lf"
    assert public.read_text() != "old helper"
    assert "warning:" not in result.stderr
    assert "/Loopflow.dmg" in (tmp_path / "curl.log").read_text()


@pytest.mark.parametrize("failure", ["LFTEST_CURL_RC", "LFTEST_PREFLIGHT_RC"])
def test_failed_custom_update_preserves_the_public_selection(
    installer: Path, env: dict[str, str], tmp_path: Path, failure: str
) -> None:
    app = tmp_path / "Custom/Loopflow.app"
    helper = app / "Contents/MacOS/lf"
    helper.parent.mkdir(parents=True)
    helper.write_text("old helper")
    public = tmp_path / "dest/lf"
    public.parent.mkdir()
    public.symlink_to(helper)
    result = _run(
        installer,
        [],
        {
            **env,
            "LF_APPLICATIONS_DIR": "",
            "LFTEST_UNAME_SYSTEM": "Darwin",
            "LFTEST_UNAME_ARCH": "arm64",
            failure: "23",
        },
    )
    assert result.returncode != 0
    assert public.resolve() == helper
    assert public.read_text() == "old helper"


def test_recovery_precedes_custom_destination_discovery(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    app = tmp_path / "Recovered/Loopflow.app"
    helper = app / "Contents/MacOS/lf"
    helper.parent.mkdir(parents=True)
    helper.write_text("recovered helper")
    public = tmp_path / "dest/lf"
    public.parent.mkdir()
    public.write_text("interrupted selection")
    result = _run(
        installer,
        [],
        {
            **env,
            "LF_APPLICATIONS_DIR": "",
            "LFTEST_UNAME_SYSTEM": "Darwin",
            "LFTEST_UNAME_ARCH": "arm64",
            "LFTEST_RECOVER_APP": str(app),
        },
    )
    assert result.returncode == 0, result.stderr
    assert public.resolve() == helper
    assert helper.read_text() != "recovered helper"


def test_failed_recovery_stops_before_a_new_promotion(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    result = _run(installer, [], {**env, "LFTEST_RECOVER_RC": "23"})
    assert result.returncode == 23
    assert not (tmp_path / "promote.log").exists()


def test_bootstrap_can_still_install_an_older_published_candidate(
    installer: Path, env: dict[str, str], tmp_path: Path
) -> None:
    result = _run(installer, [], {**env, "LFTEST_OLDER_CANDIDATE": "1"})
    assert result.returncode == 0, result.stderr
    assert (tmp_path / "dest/lf").is_file()
    assert (tmp_path / "promote.log").read_text().splitlines()[-1] == "install schedule"
