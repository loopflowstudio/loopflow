import json
import subprocess
import sys

import pytest

from scripts import test_ios


def _runtime(version: str, available: bool = True) -> dict:
    return {
        "identifier": f"com.apple.CoreSimulator.SimRuntime.iOS-{version}",
        "version": version.replace("-", "."),
        "isAvailable": available,
        "supportedDeviceTypes": [
            {"identifier": "tablet", "productFamily": "iPad"},
            {"identifier": f"phone-{version}", "productFamily": "iPhone"},
        ],
    }


def test_selects_an_iphone_supported_by_the_latest_installed_runtime():
    runtime, device = test_ios._device(
        [
            _runtime("26-1"),
            _runtime("27-0", available=False),
            _runtime("26-2"),
        ]
    )
    assert runtime.endswith("iOS-26-2")
    assert device == "phone-26-2"


def test_missing_runtime_explains_where_to_install_it():
    with pytest.raises(RuntimeError, match="Xcode Settings"):
        test_ios._device([])


def test_failed_native_test_removes_only_its_disposable_device(monkeypatch):
    devices = {"personal-phone"}
    monkeypatch.setenv("LF_HOME", "/personal/loopflow")
    monkeypatch.setattr(sys, "argv", ["test_ios.py"])
    monkeypatch.setattr(test_ios.signal, "signal", lambda *_args: None)

    def run(command, **options):
        assert "LF_HOME" not in options["env"]
        output = ""
        if command[0] == "xcodebuild":
            assert devices == {"personal-phone", "test-phone"}
            return subprocess.CompletedProcess(command, 65)
        if command[:4] == ["xcrun", "simctl", "list", "runtimes"]:
            output = json.dumps({"runtimes": [_runtime("26-2")]})
        elif command[:3] == ["xcrun", "simctl", "create"]:
            devices.add("test-phone")
            output = "test-phone\n"
        elif command[:3] == ["xcrun", "simctl", "delete"]:
            devices.remove(command[-1])
        return subprocess.CompletedProcess(command, 0, stdout=output)

    monkeypatch.setattr(test_ios.subprocess, "run", run)
    assert test_ios.main() == 65
    assert devices == {"personal-phone"}
