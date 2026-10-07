from __future__ import annotations

import importlib.util
import json
import plistlib
import shlex
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[2] / "scripts"
SCRIPT_PATH = SCRIPTS_DIR / "release-loopflow.py"
MODULE_NAME = "_release_loopflow_script_module"
spec = importlib.util.spec_from_file_location(MODULE_NAME, SCRIPT_PATH)
if spec is None or spec.loader is None:
    raise RuntimeError(f"failed to load module spec from {SCRIPT_PATH}")
release_loopflow = importlib.util.module_from_spec(spec)
sys.path.insert(0, str(SCRIPTS_DIR))
try:
    sys.modules[MODULE_NAME] = release_loopflow
    spec.loader.process_module(release_loopflow)
finally:
    sys.path.pop(0)


def test_release_bundle_renames_swift_product_to_bundle_executable(tmp_path: Path) -> None:
    build_dir = tmp_path / "build"
    build_dir.mkdir()
    (build_dir / "LoopflowMac").write_bytes(b"swift-product")

    info_plist = tmp_path / "Info.plist"
    with info_plist.open("wb") as file:
        plistlib.dump({"CFBundleExecutable": "Loopflow"}, file)

    app_macos_dir = tmp_path / "Loopflow.app" / "Contents" / "MacOS"
    app_macos_dir.mkdir(parents=True)
    release_loopflow._copy_app_executable(build_dir, info_plist, app_macos_dir)

    assert (app_macos_dir / "Loopflow").read_bytes() == b"swift-product"
    assert not (app_macos_dir / "LoopflowMac").exists()


def test_release_bundle_carries_the_cli(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    release_dir = tmp_path / "release"
    release_dir.mkdir()
    for name in ("lf",):
        (release_dir / name).write_bytes(f"release-{name}".encode())
    app_macos_dir = tmp_path / "Loopflow.app/Contents/MacOS"
    app_macos_dir.mkdir(parents=True)
    monkeypatch.setenv("LF_RELEASE_BINARY", str(release_dir / "lf"))

    release_loopflow._copy_bundled_tools(app_macos_dir)

    assert (app_macos_dir / "lf").read_bytes() == b"release-lf"
    assert not (app_macos_dir / "lfd").exists()


def _write_test_app(app_path: Path, script: str) -> None:
    app_macos_dir = app_path / "Contents" / "MacOS"
    app_macos_dir.mkdir(parents=True)
    with (app_path / "Contents" / "Info.plist").open("wb") as file:
        plistlib.dump({"CFBundleExecutable": "Loopflow"}, file)
    executable = app_macos_dir / "Loopflow"
    executable.write_text("#!/bin/sh\nset -eu\n" + script)
    executable.chmod(0o755)


def test_resource_check_hides_then_restores_build_resources(tmp_path: Path) -> None:
    build_dir = tmp_path / "build"
    build_bundle = build_dir / "LoopflowSwift_Loopflow.bundle"
    build_bundle.mkdir(parents=True)
    app_path = tmp_path / "Loopflow.app"
    quoted_bundle = shlex.quote(str(build_bundle))
    _write_test_app(
        app_path,
        f'test ! -e {quoted_bundle}\nprintf snapshot > "$LOOPFLOW_UI_TEST_SNAPSHOT_PATH"\n',
    )

    release_loopflow._verify_app_resource_self_containment(app_path, build_dir)

    assert build_bundle.is_dir()


def test_resource_check_rejects_a_build_dependent_app(tmp_path: Path) -> None:
    build_dir = tmp_path / "build"
    build_bundle = build_dir / "LoopflowSwift_Loopflow.bundle"
    build_bundle.mkdir(parents=True)
    app_path = tmp_path / "Loopflow.app"
    quoted_bundle = shlex.quote(str(build_bundle))
    _write_test_app(
        app_path,
        f'test -e {quoted_bundle}\nprintf snapshot > "$LOOPFLOW_UI_TEST_SNAPSHOT_PATH"\n',
    )

    with pytest.raises(RuntimeError, match="failed without SwiftPM build resources"):
        release_loopflow._verify_app_resource_self_containment(app_path, build_dir)

    assert build_bundle.is_dir()


def test_resource_check_requires_a_rendered_snapshot(tmp_path: Path) -> None:
    build_dir = tmp_path / "build"
    build_bundle = build_dir / "LoopflowSwift_Loopflow.bundle"
    build_bundle.mkdir(parents=True)
    app_path = tmp_path / "Loopflow.app"
    _write_test_app(app_path, "exit 0\n")

    with pytest.raises(RuntimeError, match="produced no resource-check snapshot"):
        release_loopflow._verify_app_resource_self_containment(app_path, build_dir)

    assert build_bundle.is_dir()


def test_release_command_reports_timeout(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    def time_out(cmd: list[str], **kwargs: object) -> None:
        raise subprocess.TimeoutExpired(cmd, kwargs["timeout"])

    monkeypatch.setattr(release_loopflow.subprocess, "run", time_out)

    with pytest.raises(RuntimeError, match="command timed out after 7s"):
        release_loopflow.run(["slow-command"], timeout=7)

    output = capsys.readouterr().out
    assert "$ slow-command" in output
    assert "Timed out after 7s" in output


def test_notarization_without_credentials_fails_clearly(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    for name in ("NOTARY_KEY", "NOTARY_KEY_ID", "NOTARY_ISSUER"):
        monkeypatch.delenv(name, raising=False)

    assert release_loopflow._notarize_dmg(tmp_path / "Loopflow.dmg") == 1
    assert "Missing notarization credentials" in capsys.readouterr().out


@pytest.mark.parametrize("failure", ["wait", "staple"])
def test_notarization_retry_reuses_submission_after_checkout_cleanup(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, failure: str
) -> None:
    checkout = tmp_path / "checkout"
    dmg = checkout / "swift/dist/Loopflow.dmg"
    dmg.parent.mkdir(parents=True)
    dmg.write_bytes(b"signed candidate")
    retained = tmp_path / "retained"
    monkeypatch.setattr(release_loopflow, "SWIFT_DIR", checkout / "swift")
    monkeypatch.setenv("LF_RELEASE_NOTARIZATION_DIR", str(retained))
    for name in ("NOTARY_KEY", "NOTARY_KEY_ID", "NOTARY_ISSUER"):
        monkeypatch.setenv(name, "fixture")
    submissions: list[bytes] = []
    failed = False

    def capture(cmd: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        nonlocal failed
        if cmd[0] == "git":
            return subprocess.CompletedProcess(cmd, 0, "source-commit\n", "")
        if cmd[2] == "submit":
            submissions.append(Path(cmd[3]).read_bytes())
            return subprocess.CompletedProcess(cmd, 0, '{"id":"submission-1"}', "")
        assert cmd[2:4] == ["wait", "submission-1"]
        if failure == "wait" and not failed:
            failed = True
            raise RuntimeError("wait timed out")
        return subprocess.CompletedProcess(cmd, 0, '{"status":"Accepted"}', "")

    def run(cmd: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        nonlocal failed
        assert cmd[:3] == ["xcrun", "stapler", "staple"]
        assert Path(cmd[3]).read_bytes() == b"signed candidate"
        Path(cmd[3]).write_bytes(b"stapled candidate")
        if failure == "staple" and not failed:
            failed = True
            raise RuntimeError("stapling interrupted")
        return subprocess.CompletedProcess(cmd, 0, "", "")

    monkeypatch.setattr(release_loopflow, "run_capture", capture)
    monkeypatch.setattr(release_loopflow, "run", run)
    with pytest.raises(RuntimeError):
        release_loopflow._notarize_dmg(dmg)
    shutil.rmtree(checkout)
    assert release_loopflow.release() == 0
    assert submissions == [b"signed candidate"]
    assert dmg.read_bytes() == b"stapled candidate"
    assert (retained / "Loopflow.dmg").read_bytes() == b"signed candidate"


@pytest.mark.parametrize("changed", ["artifact", "source", "unknown"])
def test_notarization_does_not_resubmit_uncertain_or_changed_artifacts(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, changed: str
) -> None:
    retained = tmp_path / "retained"
    retained.mkdir()
    (retained / "Loopflow.dmg").write_bytes(b"signed candidate")
    (retained / "submission.json").write_text(
        json.dumps(
            {
                "source_commit": "different" if changed == "source" else "source",
                "sha256": "wrong"
                if changed == "artifact"
                else release_loopflow.hashlib.sha256(b"signed candidate").hexdigest(),
                "submission_id": None if changed == "unknown" else "submission-1",
            }
        )
    )
    monkeypatch.setenv("LF_RELEASE_NOTARIZATION_DIR", str(retained))
    for name in ("NOTARY_KEY", "NOTARY_KEY_ID", "NOTARY_ISSUER"):
        monkeypatch.setenv(name, "fixture")

    def capture(cmd: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        assert cmd[0] == "git", "uncertain artifact must not be submitted"
        return subprocess.CompletedProcess(cmd, 0, "source", "")

    monkeypatch.setattr(release_loopflow, "run_capture", capture)
    with pytest.raises(RuntimeError, match="changed|different source|outcome unknown"):
        release_loopflow._notarize_dmg(tmp_path / "checkout/Loopflow.dmg")
