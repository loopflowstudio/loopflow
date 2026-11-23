import importlib.util
import plistlib
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "loopflow-dev.py"
SPEC = importlib.util.spec_from_file_location("loopflow_dev", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
loopflow_dev = importlib.util.module_from_spec(SPEC)
sys.path.insert(0, str(SCRIPT.parent))
try:
    SPEC.loader.exec_module(loopflow_dev)
finally:
    sys.path.pop(0)


@pytest.mark.parametrize("debug", [False, True])
def test_run_opens_the_machine(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str], debug: bool
) -> None:
    repo = tmp_path / "checkout"
    repo.mkdir()
    receipt = tmp_path / "open.txt"
    opener = tmp_path / "open"
    opener.write_text(
        "#!/bin/sh\n"
        f'{{ /usr/bin/env | /usr/bin/grep "^LF_" ; /usr/bin/printf "%s\\n" "$@"; }} > "{receipt}"\n'
    )
    opener.chmod(0o755)
    monkeypatch.setenv("PATH", f"{tmp_path}:/usr/bin:/bin")
    monkeypatch.setenv("LF_BIN", "/unrelated/lf")
    monkeypatch.setenv("LF_HOME", "/private/machine")
    monkeypatch.setenv("LF_CAPTURE_KEY", "enclosing-agent")
    monkeypatch.setattr(loopflow_dev, "LOOPFLOW_STREAM_LOG", tmp_path / "stream.log")
    swift = tmp_path / "swift"
    swift.write_text('#!/bin/sh\n[ "$*" = build ]\n')
    swift.chmod(0o755)
    monkeypatch.setattr(loopflow_dev, "_stop_loopflow_app", lambda _: None)
    monkeypatch.setattr(loopflow_dev, "_install_dev_app", lambda: None)

    assert loopflow_dev.cmd_run(repo=repo, debug=debug) == 0
    assert "Opening the Machine." in capsys.readouterr().out
    args = receipt.read_text().splitlines()
    assert not any(arg.startswith("LF_") for arg in args)
    assert args[-6:] == [
        "--env",
        f"LOOPFLOW_DEV_WAVE_REPO={repo}",
        str(loopflow_dev.DEV_APP),
        "--args",
        "--repo",
        str(repo),
    ]
    assert ("-W" in args) == debug


def test_dev_app_runs_under_its_own_executable_name(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    swift = tmp_path / "swift"
    (swift / ".build/debug").mkdir(parents=True)
    (swift / ".build/debug/LoopflowMac").write_bytes(b"fresh")
    (swift / "LoopflowMac").mkdir()
    (swift / "LoopflowMac/AppIcon.icns").write_bytes(b"icon")
    with (swift / "LoopflowMac/Info.plist").open("wb") as file:
        plistlib.dump(
            {"CFBundleExecutable": "Loopflow", "CFBundleIdentifier": "com.loopflow.mac"}, file
        )
    app = tmp_path / "Loopflow Dev.app"
    macos = app / "Contents/MacOS"
    macos.mkdir(parents=True)
    for stale in ("Loopflow", "LoopflowMac", "lf"):
        (macos / stale).write_bytes(b"stale")

    agents = app / "Contents/Library/LaunchAgents"
    agents.mkdir(parents=True)
    (agents / "server.plist").touch()

    def run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess:
        assert "cargo" not in command
        return subprocess.CompletedProcess(command, 0)

    monkeypatch.setattr(loopflow_dev, "SWIFT_DIR", swift)
    monkeypatch.setattr(loopflow_dev, "DEV_APP", app)
    monkeypatch.setattr(loopflow_dev, "run", run)
    monkeypatch.setattr(loopflow_dev, "_ensure_dev_signing_identity", lambda: "identity")
    # Sparkle embedding needs a real Mach-O build; test_app_bundle covers it.
    monkeypatch.setattr(sys.modules["app_bundle"], "_embed_frameworks", lambda *_: None)
    loopflow_dev._install_dev_app()

    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    assert info["CFBundleExecutable"] == "Loopflow Dev"
    assert info["CFBundleIdentifier"] == "com.loopflow.mac.dev"
    assert not agents.exists()
    assert sorted(path.name for path in macos.iterdir()) == ["Loopflow Dev"]
    assert (macos / "Loopflow Dev").read_bytes() == b"fresh"
    # The release app is untouched: its plist still names `Loopflow`.
    source = plistlib.loads((swift / "LoopflowMac/Info.plist").read_bytes())
    assert source["CFBundleExecutable"] == "Loopflow"
