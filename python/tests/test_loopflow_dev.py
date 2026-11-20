import importlib.util
import json
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


def _write_lf(path: Path, served: dict, opens: str = "any") -> None:
    """An `lf` that describes `served` and opens repositories on the Machine,
    in a world (`LF_HOME` set), or anywhere."""
    path.parent.mkdir(parents=True, exist_ok=True)
    refused = {"any": "false", "world": '[ -z "$LF_HOME" ]', "nowhere": "true"}[opens]
    path.write_text(
        "#!/bin/sh\n"
        # An app launch carries no enclosing agent.
        'if [ -n "$LF_AGENT_CALLER$LF_TASK" ]; then echo "agent context leaked" >&2; exit 9; fi\n'
        f'if [ "$1 $2" = "server describe" ]; then cat <<\'JSON\'\n{json.dumps(served)}\nJSON\n'
        "exit 0; fi\n"
        f"if {refused}; then echo 'store schema is ahead of this lf' >&2; exit 2; fi\n"
        "printf '{}\\n'\n"
    )
    path.chmod(0o755)


WIRE = {"x-revision": 8, "$defs": {"RoadmapTask": {"required": ["id"]}, "Write": {}}}
CHANGED = {"x-revision": 8, "$defs": {"RoadmapTask": {"required": ["id", "owned_by_you"]}}}


@pytest.fixture
def launcher(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    """The launcher with its build, copy and app launch replaced. Returns a
    function that runs `run` or `run-debug` and reports what was launched."""
    app = tmp_path / "Loopflow Dev.app"
    copy = tmp_path / "copy"
    installed = tmp_path / "installed"
    monkeypatch.setattr(loopflow_dev, "DEV_APP", app)
    monkeypatch.setattr(loopflow_dev, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(loopflow_dev, "COPY_DIR", copy)
    monkeypatch.setattr(loopflow_dev, "_install_dev_app", lambda: None)
    monkeypatch.setattr(loopflow_dev, "_stop_loopflow_app", lambda _: None)
    # The shell of an agent inside a world: none of it chooses what Dev opens.
    monkeypatch.setenv("LF_AGENT_CALLER", "enclosing-agent")
    monkeypatch.setenv("LF_TASK", "enclosing-task")
    monkeypatch.setenv("LF_BIN", "/unrelated/lf")
    monkeypatch.setenv("LF_HOME", str(tmp_path / "inherited-world"))
    monkeypatch.setenv("PATH", f"{installed}:/usr/bin:/bin")
    launched: dict = {}

    def copy_machine() -> bool:
        if launched.get("copy_fails"):
            print("migration 0142 failed", file=sys.stderr)
            return False
        (copy / "home").mkdir(parents=True)
        (copy / "home/loopflow.db").touch()
        (copy / "env.json").write_text(json.dumps({"LF_HOME": str(copy / "home")}))
        launched["copied"] = True
        return True

    def run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess:
        if command[0] == "open":
            pairs = [command[i + 1] for i, word in enumerate(command) if word == "--env"]
            launched["env"] = dict(pair.split("=", 1) for pair in pairs)
            launched["repo"] = command[-1]
        return subprocess.CompletedProcess(command, 0)

    def launch(*args: object, **kwargs: object) -> int:
        launched["env"] = kwargs["env"]
        launched["repo"] = kwargs["args"][-1]
        return 0

    monkeypatch.setattr(loopflow_dev, "_copy_machine", copy_machine)
    monkeypatch.setattr(loopflow_dev, "run", run)
    monkeypatch.setattr(loopflow_dev, "run_app_bundle_with_log", launch)

    def start(debug: bool, **flags: object) -> tuple[int, dict]:
        command = loopflow_dev.cmd_run_debug if debug else loopflow_dev.cmd_run
        return command(**flags), launched

    start.bundled = app / "Contents/MacOS/lf"
    start.installed = installed / "lf"
    start.copy = copy
    start.launched = launched
    return start


@pytest.mark.parametrize("debug", [False, True])
def test_a_branch_that_speaks_the_installed_wire_opens_the_machine(
    launcher, capsys: pytest.CaptureFixture[str], debug: bool
) -> None:
    _write_lf(launcher.bundled, WIRE)
    _write_lf(launcher.installed, WIRE)

    result, launched = launcher(debug)

    assert result == 0
    assert "Opening the Machine." in capsys.readouterr().out
    assert "copied" not in launched
    # The Machine, whatever `LF_HOME` the launching shell carries.
    assert "LF_HOME" not in launched["env"]
    assert launched["env"]["LF_BIN"] == str(launcher.bundled)


@pytest.mark.parametrize("debug", [False, True])
def test_a_branch_that_changes_the_wire_opens_a_copy_and_names_the_types(
    launcher, capsys: pytest.CaptureFixture[str], debug: bool
) -> None:
    _write_lf(launcher.bundled, CHANGED)
    _write_lf(launcher.installed, WIRE)

    result, launched = launcher(debug)

    assert result == 0
    said = capsys.readouterr().out
    assert "This branch changes what the server sends (RoadmapTask, Write)." in said
    assert "Opening a copy of the Machine from " in said
    assert "changes are refused there." in said
    assert launched["env"]["LF_HOME"] == str(launcher.copy / "home")
    assert launched["env"]["LF_BIN"] == str(launcher.bundled)


def test_a_store_the_branch_cannot_open_opens_a_copy_and_says_why(
    launcher, capsys: pytest.CaptureFixture[str]
) -> None:
    _write_lf(launcher.bundled, WIRE, opens="world")
    _write_lf(launcher.installed, WIRE)

    result, launched = launcher(False)

    assert result == 0
    said = capsys.readouterr().out
    assert "store schema is ahead of this lf" in said
    assert "Opening a copy of the Machine from " in said
    assert launched["env"]["LF_HOME"] == str(launcher.copy / "home")


def test_no_installed_lf_opens_a_copy_and_says_so(
    launcher, capsys: pytest.CaptureFixture[str]
) -> None:
    _write_lf(launcher.bundled, WIRE)

    result, launched = launcher(False)

    assert result == 0
    assert "No installed lf to compare this branch with." in capsys.readouterr().out
    assert launched["copied"]


def test_copy_opens_a_copy_on_a_branch_that_could_open_the_machine(
    launcher, capsys: pytest.CaptureFixture[str]
) -> None:
    _write_lf(launcher.bundled, WIRE)
    _write_lf(launcher.installed, WIRE)

    result, launched = launcher(False, copy=True)

    assert result == 0
    said = capsys.readouterr().out
    assert "Opening a copy of the Machine from " in said
    assert "This branch changes" not in said
    assert launched["env"]["LF_HOME"] == str(launcher.copy / "home")


def test_a_copy_that_cannot_be_made_launches_nothing_and_names_the_way_forward(
    launcher, capsys: pytest.CaptureFixture[str]
) -> None:
    _write_lf(launcher.bundled, WIRE, opens="nowhere")
    _write_lf(launcher.installed, WIRE)
    launcher.launched["copy_fails"] = True

    result, launched = launcher(False)

    assert result == 1
    assert "env" not in launched
    captured = capsys.readouterr()
    assert "store schema is ahead of this lf" in captured.out
    assert "migration 0142 failed" in captured.err
    assert "lf sync main" in captured.err


@pytest.mark.parametrize("debug", [False, True])
def test_world_opens_that_world_and_its_repository(
    launcher, tmp_path: Path, capsys: pytest.CaptureFixture[str], debug: bool
) -> None:
    world = tmp_path / "w1"
    (world / "repo").mkdir(parents=True)
    env = {"LF_HOME": str(world / "home"), "PATH": f"{world}/bin:/usr/bin:/bin"}
    (world / "env.json").write_text(json.dumps(env))
    _write_lf(launcher.bundled, CHANGED, opens="world")

    result, launched = launcher(debug, world=world)

    assert result == 0
    assert f"Opening the world at {world}." in capsys.readouterr().out
    assert "copied" not in launched
    assert launched["env"]["LF_HOME"] == str(world / "home")
    assert launched["env"]["PATH"] == env["PATH"]
    assert launched["repo"] == str(world / "repo")


def test_a_world_the_bundled_lf_cannot_open_launches_nothing(
    launcher, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    world = tmp_path / "w1"
    (world / "repo").mkdir(parents=True)
    (world / "env.json").write_text(json.dumps({"LF_HOME": str(world / "home")}))
    _write_lf(launcher.bundled, WIRE, opens="nowhere")

    result, launched = launcher(False, world=world)

    assert result == 1
    assert "env" not in launched
    assert "store schema is ahead of this lf" in capsys.readouterr().err


@pytest.mark.parametrize(
    "failure", [FileNotFoundError("missing helper"), subprocess.TimeoutExpired("lf", 30)]
)
def test_an_unavailable_bundled_lf_is_the_refusal(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, failure: Exception
) -> None:
    def fail(*args: object, **kwargs: object) -> None:
        raise failure

    monkeypatch.setattr(loopflow_dev.subprocess, "run", fail)
    assert loopflow_dev._repository_refusal(tmp_path) == str(failure)


@pytest.mark.skipif(sys.platform != "darwin", reason="plutil rewrites the bundle plist")
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
    for stale in ("Loopflow", "LoopflowMac"):
        (macos / stale).write_bytes(b"stale")

    real_run = loopflow_dev.run

    def run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess:
        if command[0] == "plutil":
            return real_run(command, **kwargs)
        return subprocess.CompletedProcess(command, 0)

    monkeypatch.setattr(loopflow_dev, "SWIFT_DIR", swift)
    monkeypatch.setattr(loopflow_dev, "DEV_APP", app)
    monkeypatch.setattr(loopflow_dev, "run", run)
    monkeypatch.setattr(loopflow_dev, "_copy_bundled_tools", lambda _: None)
    monkeypatch.setattr(loopflow_dev, "_ensure_dev_signing_identity", lambda: "identity")
    # Sparkle embedding needs a real Mach-O build; test_app_bundle covers it.
    monkeypatch.setattr(sys.modules["app_bundle"], "_embed_frameworks", lambda *_: None)
    loopflow_dev._install_dev_app()

    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    assert info["CFBundleExecutable"] == "Loopflow Dev"
    assert info["CFBundleIdentifier"] == "com.loopflow.mac.dev"
    assert sorted(path.name for path in macos.iterdir()) == ["Loopflow Dev"]
    assert (macos / "Loopflow Dev").read_bytes() == b"fresh"
    # The release app is untouched: its plist still names `Loopflow`.
    source = plistlib.loads((swift / "LoopflowMac/Info.plist").read_bytes())
    assert source["CFBundleExecutable"] == "Loopflow"
