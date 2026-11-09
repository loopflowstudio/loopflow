"""Run the native iOS tests on a fresh, disposable Simulator.

    uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/OwnerStreamTests

Additional arguments pass straight to xcodebuild. No personal Simulator is used.
"""

import json
import os
import signal
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def _device(runtimes: list[dict]) -> tuple[str, str]:
    available = sorted(
        (
            runtime
            for runtime in runtimes
            if runtime.get("isAvailable") and ".iOS-" in runtime["identifier"]
        ),
        key=lambda runtime: tuple(int(part) for part in runtime["version"].split(".")),
        reverse=True,
    )
    for runtime in available:
        for device in runtime["supportedDeviceTypes"]:
            if device["productFamily"] == "iPhone":
                return runtime["identifier"], device["identifier"]
    raise RuntimeError("Install an iOS Simulator runtime in Xcode Settings → Components")


def _terminate(signum: int, _frame: object) -> None:
    raise SystemExit(128 + signum)


def main() -> int:
    env = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
    subprocess.run(
        ["xcodegen", "generate", "--spec", "swift/ios.yml"], cwd=ROOT, env=env, check=True
    )
    result = subprocess.run(
        ["xcrun", "simctl", "list", "runtimes", "--json"],
        stdout=subprocess.PIPE,
        text=True,
        env=env,
        check=True,
    )
    runtime, kind = _device(json.loads(result.stdout)["runtimes"])
    result = subprocess.run(
        ["xcrun", "simctl", "create", "Loopflow iOS tests", kind, runtime],
        stdout=subprocess.PIPE,
        text=True,
        env=env,
        check=True,
    )
    device = result.stdout.strip()
    print(f"Test Simulator: {device}", flush=True)
    signal.signal(signal.SIGTERM, _terminate)
    try:
        return subprocess.run(
            [
                "xcodebuild",
                "test",
                "-project",
                "swift/LoopflowMobile.xcodeproj",
                "-scheme",
                "LoopflowiOS",
                "-destination",
                f"platform=iOS Simulator,id={device}",
                "-derivedDataPath",
                "swift/.build/ios",
                "-skipPackagePluginValidation",
                "-parallel-testing-enabled",
                "NO",
                "-jobs",
                "4",
                "CODE_SIGNING_ALLOWED=YES",
                "CODE_SIGN_IDENTITY=-",
                *sys.argv[1:],
            ],
            cwd=ROOT,
            env=env,
            stdin=subprocess.DEVNULL,
        ).returncode
    finally:
        # Xcode may already have shut it down after testing.
        subprocess.run(
            ["xcrun", "simctl", "shutdown", device],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        subprocess.run(["xcrun", "simctl", "delete", device], env=env, check=True)


if __name__ == "__main__":
    raise SystemExit(main())
