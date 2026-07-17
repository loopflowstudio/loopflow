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


def _write_fake_promotion_boundary(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        """#!/usr/bin/env python3
import os
import pathlib
import shutil
import sys

args = sys.argv[1:]
if args == ["--version"]:
    print("lf 9.9.9")
    raise SystemExit(0)
if args[:2] != ["install", "promote"]:
    raise SystemExit(2)

log = pathlib.Path(os.environ["PROMOTION_LOG"])
log.write_text(" ".join(args) + "\\n")
immutable = pathlib.Path(os.environ["PROMOTED_BINARY"])
shutil.copy2(pathlib.Path(sys.argv[0]), immutable)
immutable.chmod(0o555)

def option(name):
    return pathlib.Path(args[args.index(name) + 1])

cli_target = option("--cli-target")
cli_target.parent.mkdir(parents=True, exist_ok=True)
temporary = cli_target.with_name(".lf.fake-boundary")
temporary.unlink(missing_ok=True)
temporary.symlink_to(immutable)
temporary.replace(cli_target)

if "--app-source" in args:
    app_source = option("--app-source")
    app_target = option("--app-target")
    if app_target.exists():
        shutil.rmtree(app_target)
    shutil.copytree(app_source, app_target, symlinks=True)
    (app_target / ".rust-promotion-boundary").write_text("committed")
    legacy = option("--legacy-app-target")
    if legacy.exists():
        shutil.rmtree(legacy)
"""
    )
    path.chmod(0o755)


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


def test_only_canonical_or_tagged_installs_receive_migration_authority(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    values = {
        ("rev-parse", "HEAD"): "branch-head",
        ("symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"): "refs/remotes/origin/main",
        ("rev-parse", "refs/remotes/origin/main"): "main-head",
        ("tag", "--points-at", "HEAD"): "",
        ("status", "--porcelain"): "",
    }
    monkeypatch.setattr(
        install,
        "_git_stdout",
        lambda args, repo=install.ROOT, check=True: values[tuple(args)],
    )

    assert install._migration_authority() == "validation_only"
    values[("rev-parse", "HEAD")] = "main-head"
    assert install._migration_authority() == "published"
    values[("rev-parse", "HEAD")] = "tagged-head"
    values[("tag", "--points-at", "HEAD")] = "v0.11.2"
    assert install._migration_authority() == "published"
    values[("status", "--porcelain")] = " M rust/loopflow/src/store/migrations.rs"
    assert install._migration_authority() == "validation_only"


@pytest.mark.parametrize("no_pull", [False, True])
def test_refresh_routes_default_no_pull_and_custom_dir_through_rust_promotion(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, no_pull: bool
) -> None:
    root = tmp_path / "repo"
    candidate = root / "target" / "release" / "lf"
    _write_fake_promotion_boundary(candidate)
    install_dir = tmp_path / "custom-bin"
    install_dir.mkdir()
    (install_dir / "lf").write_text("python-direct-copy")
    log = tmp_path / "promotion.log"
    immutable = tmp_path / "immutable-lf"
    refreshed: list[Path] = []

    monkeypatch.setattr(install, "ROOT", root)
    monkeypatch.setattr(install, "_build_cli_binaries", lambda: None)
    monkeypatch.setattr(install, "_refresh_default_branch", refreshed.append)
    monkeypatch.setenv("PROMOTION_LOG", str(log))
    monkeypatch.setenv("PROMOTED_BINARY", str(immutable))

    install.refresh(no_pull=no_pull, install_dir=install_dir)

    assert (install_dir / "lf").is_symlink()
    assert (install_dir / "lf").resolve() == immutable
    command = log.read_text()
    assert command.startswith("install promote ")
    assert f"--cli-target {install_dir / 'lf'}" in command
    assert "--sync-skills" in command
    assert refreshed == ([] if no_pull else [root])


def test_local_use_routes_cli_app_bundled_helper_and_skills_through_rust_promotion(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    root = tmp_path / "repo"
    _stage_build_artifacts(root)
    _write_fake_promotion_boundary(root / "target" / "release" / "lf")
    local_bin = root / "local-bin"
    install_dir = tmp_path / "bin"
    applications = tmp_path / "Applications"
    install_dir.mkdir()
    (install_dir / "lf").write_text("python-direct-copy")
    (applications / "Loopflow.app").mkdir(parents=True)
    (applications / "Loopflow.app" / "old").write_text("old app")
    (applications / "Concerto.app").mkdir()
    log = tmp_path / "promotion.log"
    immutable = tmp_path / "immutable-lf"

    # Build the bundle spec from the tmp root before patching so local() stages
    # the app under local_bin (its default arg would otherwise bind the real ROOT).
    spec = install.default_bundle_spec(root=root)

    monkeypatch.setattr(install, "ROOT", root)
    monkeypatch.setattr(install, "LOCAL_BIN", local_bin)
    monkeypatch.setattr(install, "APPLICATIONS_DIR", applications)
    monkeypatch.setattr(install, "default_bundle_spec", lambda: spec)
    monkeypatch.setattr(install, "_resolve_install_dir", lambda: install_dir)
    monkeypatch.setattr(install, "_run_parallel_builds", lambda _skip: None)
    monkeypatch.setattr(install, "read_release_version", lambda _root: "9.9.9")
    monkeypatch.setenv("PROMOTION_LOG", str(log))
    monkeypatch.setenv("PROMOTED_BINARY", str(immutable))
    _patch_subprocess(monkeypatch)

    install.local(use=True, dry_run=False, skip=[])

    assert (install_dir / "lf").is_symlink()
    assert (install_dir / "lf").resolve() == immutable
    installed_app = applications / "Loopflow.app"
    assert (installed_app / ".rust-promotion-boundary").read_text() == "committed"
    assert (installed_app / "Contents" / "MacOS" / "lf").exists()
    assert not (applications / "Concerto.app").exists()
    command = log.read_text()
    assert f"--app-source {local_bin / 'Loopflow.app'}" in command
    assert f"--app-target {installed_app}" in command
    assert "--sync-skills" in command
