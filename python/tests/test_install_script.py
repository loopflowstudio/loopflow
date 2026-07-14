"""Tests for scripts/install.py."""

from __future__ import annotations

import importlib.util
import platform
import plistlib
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[2] / "scripts"
SCRIPT_PATH = SCRIPTS_DIR / "install.py"
MODULE_NAME = "_install_script_module"
spec = importlib.util.spec_from_file_location(MODULE_NAME, SCRIPT_PATH)
if spec is None or spec.loader is None:
    raise RuntimeError(f"failed to load module spec from {SCRIPT_PATH}")
install = importlib.util.module_from_spec(spec)
sys.path.insert(0, str(SCRIPTS_DIR))
try:
    sys.modules[MODULE_NAME] = install
    spec.loader.exec_module(install)
finally:
    sys.path.pop(0)


# --- Fixtures ---


def _write_fake_macho(path: Path, content: bytes = b"fake-macho") -> None:
    path.write_bytes(content)
    path.chmod(0o755)


def _stage_build_artifacts(root: Path) -> None:
    swift_rel = root / "swift" / ".build" / "release"
    swift_rel.mkdir(parents=True)
    _write_fake_macho(swift_rel / "LoopflowMac")

    swift_loopflow = root / "swift" / "LoopflowMac"
    swift_loopflow.mkdir(parents=True)
    (swift_loopflow / "Info.plist").write_text(
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" '
        '"http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n'
        '<plist version="1.0"><dict>'
        "<key>CFBundleName</key><string>Loopflow</string>"
        "</dict></plist>\n"
    )
    (swift_loopflow / "Loopflow.sdef").write_text("<dictionary/>")
    (swift_loopflow / "AppIcon.icns").write_bytes(b"icns-fake")

    cargo_rel = root / "target" / "release"
    cargo_rel.mkdir(parents=True)
    _write_fake_macho(cargo_rel / "lf")


def _make_spec(root: Path) -> install.BundleSpec:
    return install.default_bundle_spec(root=root)


def _stage_bundle(spec: install.BundleSpec, binaries: tuple[str, ...]) -> None:
    spec.macos_dir.mkdir(parents=True)
    spec.resources_dir.mkdir(parents=True)
    for name in binaries:
        _write_fake_macho(spec.macos_dir / name)
    (spec.contents_dir / spec.info_plist.name).write_text("<plist/>")
    for resource in spec.resources:
        (spec.resources_dir / resource.name).write_bytes(b"x")


def _patch_subprocess(
    monkeypatch: pytest.MonkeyPatch,
    *,
    archs: list[str] | None = None,
    codesign_rc: int = 0,
    codesign_verify_rc: int = 0,
) -> None:
    """Fake out lipo and codesign."""
    arch_list = archs if archs is not None else [platform.machine()]

    real_run = subprocess.run

    def fake_run(cmd, *args, **kwargs):
        if cmd and cmd[0] == "lipo" and cmd[1:2] == ["-archs"]:
            return subprocess.CompletedProcess(cmd, 0, stdout=" ".join(arch_list), stderr="")
        if cmd and cmd[0] == "codesign":
            if "--verify" in cmd:
                return subprocess.CompletedProcess(cmd, codesign_verify_rc, stdout="", stderr="")
            return subprocess.CompletedProcess(cmd, codesign_rc, stdout="", stderr="")
        if cmd and cmd[0] == "security" and cmd[1:2] == ["find-identity"]:
            # No signing identity available → ad-hoc signing (the CI/test path).
            return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")
        return real_run(cmd, *args, **kwargs)

    monkeypatch.setattr(install.subprocess, "run", fake_run)


# --- Tests ---


def test_install_loopflow_bundles_only_the_lf_helper(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    root = tmp_path / "repo"
    _stage_build_artifacts(root)
    _patch_subprocess(monkeypatch)

    spec = _make_spec(root)
    install._install_loopflow(spec, "9.9.9")

    assert (spec.macos_dir / "Loopflow").exists()
    assert (spec.macos_dir / "lf").exists()

    stamped = plistlib.loads((spec.contents_dir / "Info.plist").read_bytes())
    assert stamped["CFBundleShortVersionString"] == "9.9.9"
    assert stamped["CFBundleVersion"] == "9.9.9"


def test_verify_bundle_rejects_missing_aux_executable(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    spec = _make_spec(tmp_path / "repo")
    _stage_bundle(spec, binaries=("Loopflow",))
    _patch_subprocess(monkeypatch)

    with pytest.raises(install.StageError, match="missing:.*lf"):
        install._verify_bundle_layout(spec)


def test_verify_bundle_rejects_wrong_architecture(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    spec = _make_spec(tmp_path / "repo")
    _stage_bundle(spec, binaries=("Loopflow", "lf"))
    _patch_subprocess(monkeypatch, archs=["sparc64"])

    with pytest.raises(install.StageError, match="built for sparc64"):
        install._verify_bundle_layout(spec)


def test_verify_bundle_rejects_non_macho(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    spec = _make_spec(tmp_path / "repo")
    _stage_bundle(spec, binaries=("Loopflow", "lf"))
    _patch_subprocess(monkeypatch, archs=[])  # lipo fails -> not Mach-O

    with pytest.raises(install.StageError, match="not a Mach-O"):
        install._verify_bundle_layout(spec)


def test_install_loopflow_fails_when_codesign_verify_fails(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Signing smoke skill: if `codesign --verify` rejects the bundle,
    _install_loopflow must raise instead of silently proceeding.
    """
    root = tmp_path / "repo"
    _stage_build_artifacts(root)
    _patch_subprocess(monkeypatch, codesign_verify_rc=1)

    with pytest.raises(install.StageError, match="codesign --verify failed"):
        install._install_loopflow(_make_spec(root), "9.9.9")


def test_stage_binaries_errors_on_missing_cargo_output(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Silent-skip regression: if cargo didn't produce lf, staging into
    local-bin/ must refuse to claim success.
    """
    monkeypatch.setattr(install, "ROOT", tmp_path / "repo")
    (tmp_path / "repo" / "target" / "release").mkdir(parents=True)

    with pytest.raises(install.StageError, match="expected build artifact missing"):
        install._stage_binaries(tmp_path / "local-bin")


def test_sync_skills_runs_fresh_lf_with_yes(tmp_path: Path) -> None:
    log = tmp_path / "sync.log"
    lf = tmp_path / "lf"
    lf.write_text(f"#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" > {log}\nprintf 'synced\\n'\n")
    lf.chmod(0o755)

    install._sync_skills(lf)

    assert log.read_text() == "sync-skills --yes\n"


def test_sync_skills_warns_without_failing_when_lf_cannot_run(
    capsys: pytest.CaptureFixture[str],
) -> None:
    install._sync_skills(Path("/not/a/real/lf"))

    captured = capsys.readouterr()
    assert "skill sync failed" in captured.err
    assert "binaries installed, skills unchanged" in captured.err


def test_promote_uses_worktree_build_and_replaces_installed_app(tmp_path: Path) -> None:
    local_bin = tmp_path / "local-bin"
    local_bin.mkdir()
    (local_bin / "lf").write_text("lf")
    (local_bin / "Loopflow.app" / "Contents").mkdir(parents=True)
    (local_bin / "Loopflow.app" / "Contents" / "marker").write_text("new app")

    install_dir = tmp_path / "bin"
    install_dir.mkdir()
    (install_dir / "lf").write_text("old lf")

    applications = tmp_path / "Applications"
    (applications / "Loopflow.app").mkdir(parents=True)
    (applications / "Loopflow.app" / "old").write_text("old app")
    (applications / "Concerto.app").mkdir()

    install._promote(local_bin, install_dir, applications_dir=applications)

    assert (install_dir / "lf").is_symlink()
    assert (install_dir / "lf").readlink() == local_bin / "lf"
    assert (applications / "Loopflow.app" / "Contents" / "marker").read_text() == "new app"
    assert not (applications / "Loopflow.app" / "old").exists()
    assert not (applications / "Concerto.app").exists()
