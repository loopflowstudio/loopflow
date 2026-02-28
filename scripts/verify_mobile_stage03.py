#!/usr/bin/env python3
"""Stage 03 visual verification: foreground reconnect, cross-client clearing, action rail."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SWIFT_DIR = REPO_ROOT / "swift"


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    kwargs.setdefault("cwd", REPO_ROOT)
    kwargs.setdefault("check", False)
    return subprocess.run(cmd, **kwargs)


def _find_default_iphone_simulator() -> str:
    result = run(
        ["xcrun", "simctl", "list", "devices", "available", "-j"],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        return "iPhone 16"
    import json
    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError:
        return "iPhone 16"
    for runtime, devices in data.get("devices", {}).items():
        if "iOS" not in runtime:
            continue
        for device in devices:
            if device.get("state") == "Booted" and "iPhone" in device.get("name", ""):
                return device["name"]
    for runtime, devices in data.get("devices", {}).items():
        if "iOS" not in runtime:
            continue
        for device in devices:
            if "iPhone" in device.get("name", ""):
                return device["name"]
    return "iPhone 16"


def build_ios(device: str) -> int:
    print("Generating Xcode project...")
    result = run(["xcodegen", "generate"], cwd=SWIFT_DIR)
    if result.returncode != 0:
        print("xcodegen failed. Install with: brew install xcodegen")
        return result.returncode

    project = SWIFT_DIR / "LoopflowSwift.xcodeproj"
    destination = f"platform=iOS Simulator,name={device}"

    print(f"Building Concerto for {device}...")
    result = run([
        "xcodebuild", "build",
        "-scheme", "Concerto",
        "-project", str(project),
        "-destination", destination,
        "-configuration", "Debug",
        "CODE_SIGNING_ALLOWED=NO",
        "CODE_SIGNING_REQUIRED=NO",
    ], cwd=SWIFT_DIR)
    return result.returncode


def print_checklist() -> None:
    print()
    print("=" * 60)
    print("  Stage 03 Visual Verification Checklist")
    print("=" * 60)
    print()
    print("Prerequisites:")
    print("  - lfd running with at least one wave")
    print("  - Concerto installed on iPhone Simulator")
    print("  - Connected to lfd from the app")
    print()
    print("1. FOREGROUND RECONNECT")
    print("   a. Open a wave detail view with live output")
    print("   b. Background the app (Cmd+Shift+H)")
    print("   c. Wait 10+ seconds")
    print("   d. Foreground the app")
    print("   [ ] Connection banner does NOT appear (or clears quickly)")
    print("   [ ] Output stream resumes with current data")
    print("   [ ] Session chat shows current messages")
    print()
    print("2. CROSS-CLIENT ACTION CLEARING")
    print("   a. On iPhone, open a wave with suggested action buttons")
    print("   b. From another client (macOS Concerto or lfq), send a message")
    print("   c. This starts a new turn on the server")
    print("   [ ] Action buttons on iPhone disappear when turnStarted arrives")
    print()
    print("3. ACTION RAIL SPACING")
    print("   a. Open a wave with suggested actions visible")
    print("   [ ] Action buttons have 44pt touch targets")
    print("   [ ] Buttons are horizontally scrollable if they overflow")
    print("   [ ] Buttons sit in the safe area inset at the bottom")
    print()
    print("4. OUTPUT STREAM RESTART ON FOREGROUND")
    print("   a. Open output tab for an active wave")
    print("   b. Background the app, wait 10s, foreground")
    print("   [ ] Output refreshes (brief flash is acceptable)")
    print("   [ ] No duplicate lines appear")
    print()
    print("=" * 60)


def main() -> int:
    device = _find_default_iphone_simulator()

    rc = build_ios(device)
    if rc != 0:
        print(f"\nBuild failed (exit {rc}). Fix build errors and retry.")
        return rc

    print_checklist()
    return 0


if __name__ == "__main__":
    sys.exit(main())
