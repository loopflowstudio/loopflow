#!/usr/bin/env python3
"""Build, sign, notarize, and package Concerto as a DMG.

Usage:
    python3 scripts/release-concerto.py

Expects to run from the repo root. On CI, signing credentials come from
environment variables (NOTARY_KEY, NOTARY_KEY_ID, NOTARY_ISSUER). Locally,
it uses whatever Developer ID Application identity is in the keychain.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
SWIFT_DIR = REPO_ROOT / "swift"


def run(
    cmd: list[str], cwd: Path | None = None, check: bool = True
) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=cwd, check=check)


def run_capture(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)


def _detect_signing_identity() -> str | None:
    result = run_capture(["security", "find-identity", "-v", "-p", "codesigning"])
    for line in result.stdout.splitlines():
        if "Developer ID Application" in line:
            start = line.find('"')
            end = line.rfind('"')
            if start != -1 and end > start:
                return line[start + 1 : end]
    return None


def _codesign_app(app_path: Path, identity: str, entitlements: Path | None = None) -> int:
    cmd = [
        "codesign", "--force", "--deep", "--sign", identity,
        "--options", "runtime",
        "--timestamp",
    ]
    if entitlements and entitlements.exists():
        cmd += ["--entitlements", str(entitlements)]
    cmd.append(str(app_path))
    print(f"Signing with: {identity}")
    result = run(cmd, check=False)
    return result.returncode


def _notarize_dmg(dmg_path: Path) -> int:
    key = os.environ.get("NOTARY_KEY")
    key_id = os.environ.get("NOTARY_KEY_ID")
    issuer = os.environ.get("NOTARY_ISSUER")

    if not all([key, key_id, issuer]):
        print("Skipping notarization (NOTARY_KEY, NOTARY_KEY_ID, NOTARY_ISSUER not set)")
        return 0

    with tempfile.NamedTemporaryFile(mode="w", suffix=".p8", delete=False) as f:
        f.write(key)
        key_path = f.name

    try:
        print("Submitting for notarization...")
        result = run([
            "xcrun", "notarytool", "submit", str(dmg_path),
            "--key", key_path,
            "--key-id", key_id,
            "--issuer", issuer,
            "--wait",
        ], check=False)
        if result.returncode != 0:
            return result.returncode

        print("Stapling notarization ticket...")
        result = run(["xcrun", "stapler", "staple", str(dmg_path)], check=False)
        return result.returncode
    finally:
        os.unlink(key_path)


def _copy_bundled_tools(app_macos_dir: Path) -> None:
    cargo_cmd = ["cargo", "build", "--release", "--bin", "lf", "--bin", "lfd"]
    bin_dir = REPO_ROOT / "target" / "release"

    result = run(cargo_cmd, cwd=REPO_ROOT, check=False)
    if result.returncode != 0:
        raise RuntimeError("Failed to build bundled lf/lfd binaries")

    for binary in ("lf", "lfd"):
        source = bin_dir / binary
        if not source.exists():
            raise RuntimeError(f"Missing built binary: {source}")
        shutil.copy(source, app_macos_dir / binary)


def release() -> int:
    print("Building Concerto release...")

    result = run(["swift", "build", "-c", "release"], cwd=SWIFT_DIR, check=False)
    if result.returncode != 0:
        return result.returncode

    app_name = "Loopflow Concerto"
    dist_dir = SWIFT_DIR / "dist"
    app_dir = dist_dir / f"{app_name}.app" / "Contents"

    if dist_dir.exists():
        shutil.rmtree(dist_dir)
    (app_dir / "MacOS").mkdir(parents=True)
    (app_dir / "Resources").mkdir(parents=True)

    build_dir = SWIFT_DIR / ".build" / "release"
    shutil.copy(build_dir / "Concerto", app_dir / "MacOS")
    shutil.copy(SWIFT_DIR / "Concerto" / "Info.plist", app_dir)
    shutil.copy(SWIFT_DIR / "Concerto" / "Concerto.sdef", app_dir / "Resources")
    shutil.copy(SWIFT_DIR / "Concerto" / "AppIcon.icns", app_dir / "Resources")
    _copy_bundled_tools(app_dir / "MacOS")
    (app_dir / "PkgInfo").write_text("APPL????")

    # Copy SPM resource bundles (fonts, etc.) into Contents/Resources/
    for bundle in build_dir.glob("*.bundle"):
        shutil.copytree(bundle, app_dir / "Resources" / bundle.name)

    print(f"Created dist/{app_name}.app")

    # Codesign
    identity = _detect_signing_identity()
    if identity:
        entitlements = SWIFT_DIR / "Concerto" / "Concerto.entitlements"
        app_path = dist_dir / f"{app_name}.app"
        rc = _codesign_app(app_path, identity, entitlements)
        if rc != 0:
            print("Codesigning failed")
            return rc
    else:
        print("No Developer ID found — signing ad-hoc (DMG will trigger Gatekeeper)")
        run(["codesign", "--force", "--deep", "--sign", "-",
             str(dist_dir / f"{app_name}.app")])

    # Create DMG
    dmg_path = dist_dir / "LoopflowConcerto.dmg"
    dmg_staging = dist_dir / "dmg_staging"
    dmg_staging.mkdir()
    shutil.copytree(dist_dir / f"{app_name}.app", dmg_staging / f"{app_name}.app")

    bg_image = SWIFT_DIR / "Concerto" / "dmg-background.png"
    has_create_dmg = shutil.which("create-dmg") is not None

    if has_create_dmg and bg_image.exists():
        cmd = [
            "create-dmg",
            "--volname", app_name,
            "--window-pos", "200", "120",
            "--window-size", "660", "400",
            "--icon-size", "128",
            "--icon", f"{app_name}.app", "180", "185",
            "--app-drop-link", "480", "185",
            "--background", str(bg_image),
            "--hide-extension", f"{app_name}.app",
            "--no-internet-enable",
            str(dmg_path),
            str(dmg_staging),
        ]
        result = run(cmd, check=False)
        if result.returncode not in (0, 2):
            shutil.rmtree(dmg_staging)
            return result.returncode
    else:
        (dmg_staging / "Applications").symlink_to("/Applications")
        run([
            "hdiutil", "create",
            "-volname", app_name,
            "-srcfolder", str(dmg_staging),
            "-ov", "-format", "UDZO",
            str(dmg_path),
        ])

    shutil.rmtree(dmg_staging)

    # Sign and notarize the DMG
    if identity:
        run(["codesign", "--force", "--sign", identity, "--timestamp", str(dmg_path)])
        rc = _notarize_dmg(dmg_path)
        if rc != 0:
            print("Notarization failed")
            return rc

    print()
    print("Release built:")
    print(f"  App: dist/{app_name}.app")
    print(f"  DMG: {dmg_path}")
    return 0


if __name__ == "__main__":
    sys.exit(release())
