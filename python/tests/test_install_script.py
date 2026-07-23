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
    _write_fake_macho(cargo_rel / "lfd")


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


def test_install_loopflow_bundles_the_control_plane_helpers(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    root = tmp_path / "repo"
    _stage_build_artifacts(root)
    _patch_subprocess(monkeypatch)

    spec = _make_spec(root)
    install._install_loopflow(spec, "9.9.9")

    assert (spec.macos_dir / "Loopflow").exists()
    assert (spec.macos_dir / "lf").exists()
    assert (spec.macos_dir / "lfd").exists()

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
    _stage_bundle(spec, binaries=("Loopflow", "lf", "lfd"))
    _patch_subprocess(monkeypatch, archs=["sparc64"])

    with pytest.raises(install.StageError, match="built for sparc64"):
        install._verify_bundle_layout(spec)


def test_verify_bundle_rejects_non_macho(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    spec = _make_spec(tmp_path / "repo")
    _stage_bundle(spec, binaries=("Loopflow", "lf", "lfd"))
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


def test_source_builds_are_always_development_validation_only(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("LOOPFLOW_BUILD_PROVENANCE", "release")
    monkeypatch.setenv("LOOPFLOW_MIGRATION_AUTHORITY", "published")

    env = install._development_build_env()

    assert env["LOOPFLOW_BUILD_PROVENANCE"] == "development"
    assert env["LOOPFLOW_MIGRATION_AUTHORITY"] == "validation_only"


def test_published_refresh_pins_and_verifies_the_external_installer(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    installer_payload = b"#!/bin/sh\nexit 0\n"
    digest = install.hashlib.sha256(installer_payload).hexdigest()
    downloads: list[str] = []
    runs: list[tuple[list[str], dict[str, str]]] = []

    def download(url: str, destination: Path) -> str:
        downloads.append(url)
        if destination.name == "SHA256SUMS":
            destination.write_text(f"{digest}  install.sh\n")
            return "https://release-assets.githubusercontent.com/signed-checksums"
        destination.write_bytes(installer_payload)
        return "https://release-assets.githubusercontent.com/signed-installer"

    monkeypatch.setattr(install, "_latest_release_tag", lambda: "v9.9.9")
    monkeypatch.setattr(install, "_download_release_asset", download)
    monkeypatch.setattr(
        install,
        "_run_or_raise",
        lambda command, _label, cwd=None, env=None: runs.append((command, env)),
    )

    tag = install._install_published_release(tmp_path / "bin")

    assert tag == "v9.9.9"
    assert downloads == [
        f"{install.RELEASE_DOWNLOAD_BASE}/v9.9.9/SHA256SUMS",
        f"{install.RELEASE_DOWNLOAD_BASE}/v9.9.9/install.sh",
    ]
    assert runs[0][0][0] == "sh"
    assert runs[0][0][-2:] == ["--version", "v9.9.9"]
    assert runs[0][1]["LF_INSTALL_DIR"] == str(tmp_path / "bin")


def test_published_refresh_rejects_an_installer_digest_mismatch(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    def download(url: str, destination: Path) -> str:
        if destination.name == "SHA256SUMS":
            destination.write_text(f"{'0' * 64}  install.sh\n")
            return "https://release-assets.githubusercontent.com/signed-checksums"
        destination.write_text("different")
        return "https://release-assets.githubusercontent.com/signed-installer"

    monkeypatch.setattr(install, "_latest_release_tag", lambda: "v9.9.9")
    monkeypatch.setattr(install, "_download_release_asset", download)
    monkeypatch.setattr(
        install,
        "_run_or_raise",
        lambda *_args, **_kwargs: pytest.fail("unverified installer must not execute"),
    )

    with pytest.raises(install.StageError, match="digest mismatch"):
        install._install_published_release(tmp_path / "bin")


def test_refresh_uses_only_the_published_release_path(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    install_dir = tmp_path / "bin"
    install_dir.mkdir()
    target = install_dir / "lf"
    target.write_text("#!/bin/sh\nexit 0\n")
    target.chmod(0o755)
    calls: list[Path] = []
    monkeypatch.setattr(
        install,
        "_install_published_release",
        lambda destination: calls.append(destination) or "v9.9.9",
    )

    install.refresh(install_dir=install_dir)

    assert calls == [install_dir]
    assert "release: v9.9.9" in capsys.readouterr().out


def test_local_dry_run_has_no_production_activation_path(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    root = tmp_path / "repo"
    bundle_spec = install.default_bundle_spec(root)
    monkeypatch.setattr(install, "ROOT", root)
    monkeypatch.setattr(install, "LOCAL_BIN", root / "local-bin")
    monkeypatch.setattr(install, "default_bundle_spec", lambda: bundle_spec)
    monkeypatch.setattr(install, "read_release_version", lambda _root: "9.9.9")

    install.local(dry_run=True, skip=[])

    output = capsys.readouterr().out
    assert "validation-only build under local-bin" in output
    assert "promote" not in output.lower()
