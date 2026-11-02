import importlib.util
import shlex
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
@pytest.mark.parametrize("private_home,supported", [(False, False), (False, True), (True, True)])
def test_demo_launch_requires_work_opening_on_selected_runtime(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    debug: bool,
    private_home: bool,
    supported: bool,
) -> None:
    app = tmp_path / "Loopflow Dev.app"
    cli = app / "Contents/MacOS/lf"
    cli.parent.mkdir(parents=True)
    selected_home = str(tmp_path / "private-machine") if private_home else ""
    # Source help succeeds even when main-Machine dispatch cannot open Work.
    cli.write_text(
        "#!/bin/sh\n"
        'case "$*" in *--help*) exit 0;; esac\n'
        'if [ -n "$LF_AGENT_CALLER$LF_TASK" ]; then exit 9; fi\n'
        'test "$LF_BIN" = "$0" || exit 9\n'
        f'test "$LF_HOME" = {shlex.quote(selected_home)} || exit 9\n'
        f'if [ "{supported}" = "True" ]; then printf \'{{}}\\n\'; exit 0; fi\n'
        "echo \"error: unrecognized subcommand 'identity'\" >&2\n"
        "exit 2\n"
    )
    cli.chmod(0o755)
    assert subprocess.run([str(cli), "config", "repository", "--help"]).returncode == 0

    monkeypatch.setattr(loopflow_dev, "DEV_APP", app)
    monkeypatch.setattr(loopflow_dev, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(loopflow_dev, "_install_dev_app", lambda: None)
    monkeypatch.setattr(loopflow_dev, "_stop_loopflow_app", lambda _: None)
    monkeypatch.setenv("LF_AGENT_CALLER", "enclosing-agent")
    monkeypatch.setenv("LF_TASK", "enclosing-task")
    monkeypatch.setenv("LF_BIN", "/unrelated/lf")
    if private_home:
        monkeypatch.setenv("LF_HOME", str(tmp_path / "private-machine"))
    else:
        monkeypatch.delenv("LF_HOME", raising=False)

    launched = tmp_path / "launched"

    def run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess:
        if command[0] == "open":
            launched.touch()
        return subprocess.CompletedProcess(command, 0)

    def launch(*args: object, **kwargs: object) -> int:
        launched.touch()
        return 0

    monkeypatch.setattr(loopflow_dev, "run", run)
    monkeypatch.setattr(loopflow_dev, "run_app_bundle_with_log", launch)
    result = loopflow_dev.cmd_run_debug(tmp_path) if debug else loopflow_dev.cmd_run()

    assert result == (0 if supported else 1)
    assert launched.exists() == supported
    if not supported:
        error = capsys.readouterr().err
        assert "unrecognized subcommand 'identity'" in error
        assert "bundled CLI" in error


@pytest.mark.parametrize(
    "failure", [FileNotFoundError("missing helper"), subprocess.TimeoutExpired("lf", 30)]
)
def test_repository_opening_reports_unavailable_helper(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    failure: Exception,
) -> None:
    def fail(*args: object, **kwargs: object) -> None:
        raise failure

    monkeypatch.setattr(loopflow_dev.subprocess, "run", fail)
    assert not loopflow_dev._check_repository_opening(tmp_path)
    assert str(failure) in capsys.readouterr().err
