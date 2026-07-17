"""The full local gate is bounded and honest.

No phase can hang the gate indefinitely, a timeout is reported as a named
actionable failure, the required UI host gate never runs under --all, and a
simulated runner-bootstrap failure names the missing capability.
"""

import importlib.util
import json
import re
import shutil
import sys
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/test.py"

_spec = importlib.util.spec_from_file_location("gate_test_runner", SCRIPT)
assert _spec is not None and _spec.loader is not None
gate = importlib.util.module_from_spec(_spec)
sys.modules["gate_test_runner"] = gate
_spec.loader.exec_module(gate)


def _cmd(argv: list[str], label: str) -> "gate.Command":
    return gate.Command(argv=argv, cwd=ROOT, label=label)


def _plan(*commands: "gate.Command") -> "gate.Plan":
    suite = gate.Suite(
        name="probe",
        slow=False,
        trigger_desc="test fixture",
        match=lambda _changed: True,
        build=lambda _changed: list(commands),
    )
    return gate.Plan(suite=suite, run=True, reason="test fixture", commands=list(commands))


def _history_run(
    root: Path,
    started: datetime,
    *,
    branch: str = "test/branch",
    status: str = "passed",
    phase_status: str = "passed",
    over_budget: bool = False,
) -> Path:
    run_id = f"{started.strftime('%Y%m%dT%H%M%SZ')}-1-abcdef12"
    phase = gate.PhaseOutcome(
        suite="python",
        phase="python",
        budget_s=600,
        elapsed_s=601.0 if over_budget else 12.5,
        status=phase_status,
        over_budget=over_budget,
    )
    run = gate.GateRun(
        run_id=run_id,
        kind="full",
        branch=branch,
        head="0123456789abcdef",
        task_session_id=None,
        started_at=gate._format_timestamp(started),
        finished_at=None
        if status == "running"
        else gate._format_timestamp(started + timedelta(minutes=1)),
        status=status,
        phases=[phase],
    )
    path = root / "full" / f"{run_id}.json"
    gate._write_run_record(path, run.as_record())
    return path


def _fail_write(_path: Path, _record: dict[str, object]) -> None:
    raise OSError("read-only fixture")


def test_hanging_phase_is_killed_at_its_budget(tmp_path, monkeypatch):
    # A phase that would sleep far past its budget must be killed promptly.
    monkeypatch.setitem(gate.PHASE_BUDGETS, "hang-probe", 1)
    started = time.monotonic()
    outcome = gate._run_command(_cmd(["sleep", "60"], "hang-probe"), tmp_path, "probe")
    elapsed = time.monotonic() - started

    assert outcome.status == "timed_out"
    assert outcome.over_budget is True
    assert outcome.failure is not None
    assert "TIMEOUT" in outcome.failure and "hang-probe" in outcome.failure
    # Killed near its 1s budget plus the SIGTERM->SIGKILL grace, nowhere near 60s.
    assert elapsed < 20, f"phase took {elapsed:.1f}s; the budget did not bound it"


def test_child_process_dies_with_the_group(tmp_path, monkeypatch):
    # A phase that backgrounds a long sleeper must not leave the sleeper alive:
    # the group-kill reaps children too. The marker file would be written only
    # if the child outlived the kill.
    monkeypatch.setitem(gate.PHASE_BUDGETS, "group-probe", 1)
    marker = tmp_path / "child-survived"
    script = f"sleep 30 && touch {marker} & wait"
    gate._run_command(_cmd(["bash", "-c", script], "group-probe"), tmp_path, "probe")
    time.sleep(3)
    assert not marker.exists(), "backgrounded child outlived the group-kill"


def test_failing_phase_reports_actionable_failure(tmp_path):
    outcome = gate._run_command(_cmd(["bash", "-c", "exit 3"], "red-probe"), tmp_path, "probe")
    assert outcome.status == "failed"
    assert outcome.failure is not None
    assert "FAILED" in outcome.failure and "red-probe" in outcome.failure
    assert "exit 3" in outcome.failure


def test_passing_phase_returns_measured_outcome(tmp_path):
    outcome = gate._run_command(_cmd(["bash", "-c", "exit 0"], "ok-probe"), tmp_path, "probe")
    assert outcome.status == "passed"
    assert outcome.elapsed_s >= 0
    assert outcome.budget_s == gate.DEFAULT_BUDGET_S
    assert outcome.failure is None


def test_missing_tool_is_named_not_a_traceback(tmp_path):
    outcome = gate._run_command(
        _cmd(["definitely-not-a-real-binary-xyz"], "tool-probe"), tmp_path, "probe"
    )
    assert outcome.status == "missing_tool"
    assert outcome.failure is not None
    assert "MISSING TOOL" in outcome.failure


def test_gate_history_uses_the_git_common_directory():
    common_dir = Path(
        gate.subprocess.run(
            ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )
    assert gate._gate_history_root() == common_dir / "loopflow" / "pre-land" / "runs"
    assert ROOT / ".lf" / "tmp" not in gate._gate_history_root().parents


def test_run_persists_every_selected_phase_and_skips_after_failure(tmp_path, monkeypatch, capsys):
    history_root = tmp_path / "common-history"
    marker = tmp_path / "later-phase-ran"
    monkeypatch.setattr(gate, "_gate_history_root", lambda: history_root)
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")
    plan = _plan(
        _cmd(["bash", "-c", "exit 3"], "first"),
        _cmd(["bash", "-c", f"touch {marker}"], "second"),
    )

    assert gate.run_plans([plan], kind="changed") == 1

    records = list((history_root / "changed").glob("*.json"))
    assert len(records) == 1
    record = json.loads(records[0].read_text())
    assert record["status"] == "failed"
    assert [phase["status"] for phase in record["phases"]] == ["failed", "not_run"]
    assert all("budget_s" in phase and "elapsed_s" in phase for phase in record["phases"])
    assert not marker.exists()
    summary = capsys.readouterr().out
    assert "probe/first" in summary
    assert "probe/second" in summary
    assert "not run / 900s budget" in summary


def test_initial_checkpoint_is_running_with_every_phase_not_run(tmp_path, monkeypatch):
    history_root = tmp_path / "history"
    monkeypatch.setattr(gate, "_gate_history_root", lambda: history_root)
    plan = _plan(_cmd(["true"], "first"), _cmd(["true"], "second"))

    recorder = gate._start_recorder("full", [plan])

    assert recorder is not None
    record = json.loads(recorder.path.read_text())
    assert record["status"] == "running"
    assert record["finished_at"] is None
    assert [phase["status"] for phase in record["phases"]] == ["not_run", "not_run"]
    assert not list(recorder.path.parent.glob("*.tmp"))


def test_full_measurement_failure_stops_before_verification(tmp_path, monkeypatch, capsys):
    marker = tmp_path / "phase-ran"
    monkeypatch.setattr(gate, "_gate_history_root", lambda: tmp_path / "history")
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")
    monkeypatch.setattr(gate, "_write_run_record", _fail_write)

    result = gate.run_plans(
        [_plan(_cmd(["bash", "-c", f"touch {marker}"], "probe"))],
        kind="full",
    )

    assert result == 1
    assert not marker.exists()
    assert "MEASUREMENT FAILED" in capsys.readouterr().err


def test_full_checkpoint_failure_stops_after_the_measured_phase(tmp_path, monkeypatch, capsys):
    marker = tmp_path / "phase-ran"
    history_root = tmp_path / "history"
    real_write = gate._write_run_record
    monkeypatch.setattr(gate, "_gate_history_root", lambda: history_root)
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")

    def _fail_after_initial(path: Path, record: dict[str, object]) -> None:
        if path.exists():
            raise OSError("checkpoint fixture")
        real_write(path, record)

    monkeypatch.setattr(gate, "_write_run_record", _fail_after_initial)
    result = gate.run_plans(
        [_plan(_cmd(["bash", "-c", f"touch {marker}"], "probe"))],
        kind="full",
    )

    assert result == 1
    assert marker.exists()
    record = json.loads(next((history_root / "full").glob("*.json")).read_text())
    assert record["status"] == "running"
    assert record["phases"][0]["status"] == "not_run"
    assert "MEASUREMENT FAILED" in capsys.readouterr().err


def test_changed_measurement_failure_warns_once_and_keeps_result(tmp_path, monkeypatch, capsys):
    marker = tmp_path / "phase-ran"
    monkeypatch.setattr(gate, "_gate_history_root", lambda: tmp_path / "history")
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")
    monkeypatch.setattr(gate, "_write_run_record", _fail_write)

    result = gate.run_plans(
        [_plan(_cmd(["bash", "-c", f"touch {marker}"], "probe"))],
        kind="changed",
    )

    assert result == 0
    assert marker.exists()
    assert capsys.readouterr().err.count("MEASUREMENT WARNING") == 1


def test_changed_measurement_warning_preserves_a_failing_result(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(gate, "_gate_history_root", lambda: tmp_path / "history")
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")
    monkeypatch.setattr(gate, "_write_run_record", _fail_write)

    result = gate.run_plans(
        [_plan(_cmd(["bash", "-c", "exit 7"], "probe"))],
        kind="changed",
    )

    assert result == 1
    assert capsys.readouterr().err.count("MEASUREMENT WARNING") == 1


def test_history_verdict_moves_from_progress_to_holding_to_overrun(tmp_path, monkeypatch, capsys):
    history_root = tmp_path / "history"
    monkeypatch.setattr(gate, "_gate_history_root", lambda: history_root)
    now = datetime(2026, 7, 17, 12, tzinfo=timezone.utc)

    _history_run(history_root, now - timedelta(days=29))
    assert gate._history_report(30, now=now).verdict == "IN PROGRESS"

    _history_run(history_root, now - timedelta(days=30), branch="test/clock-start")
    assert gate._history_report(30, now=now).verdict == "HOLDING"

    _history_run(
        history_root,
        now - timedelta(days=1),
        branch="test/overrun",
        status="failed",
        phase_status="timed_out",
        over_budget=True,
    )
    report = gate._history_report(30, now=now)
    assert report.verdict == "NOT HOLDING"
    assert any(
        "OVER BUDGET python/python" in issue for entry in report.entries for issue in entry.issues
    )
    gate._print_history(report)
    output = capsys.readouterr().out
    assert "Verdict: NOT HOLDING" in output
    assert "OVER BUDGET python/python" in output


def test_corrupt_full_is_a_gap_but_corrupt_changed_is_not(tmp_path, monkeypatch):
    history_root = tmp_path / "history"
    monkeypatch.setattr(gate, "_gate_history_root", lambda: history_root)
    now = datetime(2026, 7, 17, 12, tzinfo=timezone.utc)
    _history_run(history_root, now - timedelta(days=30))
    changed = history_root / "changed" / "20260716T120000Z-1-bad.json"
    changed.parent.mkdir(parents=True)
    changed.write_text("{broken")

    assert gate._history_report(30, now=now).verdict == "HOLDING"

    full = history_root / "full" / "20260716T120000Z-1-bad.json"
    full.write_text("{broken")
    assert gate._history_report(30, now=now).verdict == "NOT HOLDING"


def test_history_cli_is_read_only_and_prints_the_verdict(tmp_path, monkeypatch, capsys):
    history_root = tmp_path / "history"
    monkeypatch.setattr(gate, "_gate_history_root", lambda: history_root)

    assert gate.main(["--history", "30"]) == 0

    output = capsys.readouterr().out
    assert "Verdict: IN PROGRESS" in output
    assert not history_root.exists()


def test_history_survives_worktree_tmp_and_worktree_removal(tmp_path, monkeypatch):
    history_root = tmp_path / "common-git" / "loopflow" / "pre-land" / "runs"
    monkeypatch.setattr(gate, "_gate_history_root", lambda: history_root)
    now = datetime(2026, 7, 17, 12, tzinfo=timezone.utc)
    worktree_a = tmp_path / "repo.task-a"
    worktree_b = tmp_path / "repo.task-b"
    (worktree_a / ".lf" / "tmp").mkdir(parents=True)
    (worktree_b / ".lf" / "tmp").mkdir(parents=True)
    _history_run(history_root, now - timedelta(days=30), branch="task-a")
    _history_run(history_root, now - timedelta(days=29), branch="task-b")
    before = gate._history_report(30, now=now)

    shutil.rmtree(worktree_a)
    shutil.rmtree(worktree_b / ".lf" / "tmp")
    after = gate._history_report(30, now=now)

    assert len(before.entries) == len(after.entries) == 2
    assert before.verdict == after.verdict == "HOLDING"


def test_persisted_schema_contains_only_operational_facts():
    run = gate.GateRun(
        run_id="20260717T120000Z-1-abcdef12",
        kind="full",
        branch="test/branch",
        head="0123456789abcdef",
        task_session_id="ts_test",
        started_at="2026-07-17T12:00:00Z",
        finished_at="2026-07-17T12:01:00Z",
        status="passed",
        phases=[gate.PhaseOutcome("python", "python", 600, 12.5, "passed", False)],
    )
    record = run.as_record()

    assert set(record) == {
        "schema",
        "run_id",
        "kind",
        "branch",
        "head",
        "task_session_id",
        "started_at",
        "finished_at",
        "status",
        "phases",
    }
    assert set(record["phases"][0]) == {
        "suite",
        "phase",
        "budget_s",
        "elapsed_s",
        "status",
        "over_budget",
    }
    forbidden = {"argv", "cwd", "output", "environment", "prompt", "diff"}
    assert forbidden.isdisjoint(json.dumps(record).lower().split('"'))


def test_budget_document_names_every_executable_phase_budget():
    document = (ROOT / "release/GATE_BUDGET.md").read_text()
    for label, budget in gate.PHASE_BUDGETS.items():
        row = rf"\|\s*[^|]+\|\s*{re.escape(label)}\s*\|\s*{budget}s\s*\|"
        assert re.search(row, document), f"{label}={budget}s is absent from GATE_BUDGET.md"


def test_all_never_runs_the_required_host_gate():
    plans = gate.build_plan(changed=[], run_all=True, forced=set())
    ui = next(p for p in plans if p.suite.name == "ui-host")
    assert ui.run is False
    assert "required host gate" in ui.reason


def test_ui_host_runs_only_when_named():
    plans = gate.build_plan(changed=[], run_all=False, forced={"ui-host"})
    ui = next(p for p in plans if p.suite.name == "ui-host")
    assert ui.run is True


def test_simulated_bootstrap_failure_names_capability_and_action(monkeypatch):
    monkeypatch.setenv("LF_UI_HOST_SIMULATE_NO_PERMISSION", "1")
    gap = gate._ui_host_precheck()
    assert gap is not None
    assert "MISSING CAPABILITY" in gap
    assert "NEXT ACTION" in gap
    assert "UI_HOST_GATE.md" in gap


def test_ui_host_classify_recognises_a_runner_bootstrap_failure():
    log = "... Test runner never began executing tests after launching.\n"
    refined = gate._ui_host_classify(log)
    assert refined is not None and "MISSING CAPABILITY" in refined


def test_ui_host_classify_keeps_a_genuine_assertion_failure_raw():
    log = "XCTAssertEqual failed: ('a') is not equal to ('b')\n"
    assert gate._ui_host_classify(log) is None


def test_ui_host_classify_recognises_a_hung_control_session():
    # macOS 26 / Xcode 26 denial signature: the runner launches but hangs before
    # connecting, so LoopflowUITests never executes. Observed on the real host
    # (exit 65, ~710s hang). Must classify as a capability gap, not a red test.
    log = (
        "Timed out after 120.0s while initiating control session with daemon.\n"
        "Testing failed:\n"
        "\tLoopflowUITests-Runner (37114) encountered an error "
        "(The test runner hung before establishing connection.)\n"
        "** TEST FAILED **\n"
    )
    refined = gate._ui_host_classify(log)
    assert refined is not None and "MISSING CAPABILITY" in refined


def test_ui_host_result_bundle_is_per_run_not_a_fixed_path():
    # xcodebuild test exits 64 rather than overwrite an existing -resultBundlePath,
    # so a fixed path made the second --ui-host run onward die before launching a
    # test. The bundle must live under this run's pid-scoped artifact dir.
    cmds = gate._ui_host_commands([])
    test_cmd = next(c for c in cmds if c.label == "ui-host")
    idx = test_cmd.argv.index("-resultBundlePath")
    bundle = Path(test_cmd.argv[idx + 1])
    assert bundle.parent.parent == gate._run_artifact_root()
    assert bundle != ROOT / gate.XCODE_DERIVED_DATA / "ui-host.xcresult"
    assert str(gate._run_artifact_root()).endswith(f"run-{gate.os.getpid()}")


def test_loopflow_summary_says_it_does_not_run_hosted_ui():
    loopflow = next(s for s in gate.SUITES if s.name == "loopflow")
    assert loopflow.proves is not None
    assert "NOT run here" in loopflow.proves
