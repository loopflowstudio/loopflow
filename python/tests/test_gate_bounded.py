"""The full local gate is bounded and honest.

No phase can hang the gate indefinitely, a timeout is reported as a named
actionable failure, the required UI host gate never runs under --all, and a
simulated runner-bootstrap failure names the missing capability.
"""

import importlib.util
import json
import sys
import time
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


def test_gate_evidence_uses_the_git_common_directory():
    common_dir = Path(
        gate.subprocess.run(
            ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )
    assert gate._gate_evidence_root() == common_dir / "loopflow" / "pre-land" / "runs"
    assert ROOT / ".lf" / "tmp" not in gate._gate_evidence_root().parents


def test_run_persists_every_selected_phase_and_skips_after_failure(tmp_path, monkeypatch, capsys):
    evidence_root = tmp_path / "common-evidence"
    marker = tmp_path / "later-phase-ran"
    monkeypatch.setattr(gate, "_gate_evidence_root", lambda: evidence_root)
    monkeypatch.setattr(gate, "_tree_fingerprint", lambda: "tree")
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")
    plan = _plan(
        _cmd(["bash", "-c", "exit 3"], "first"),
        _cmd(["bash", "-c", f"touch {marker}"], "second"),
    )

    assert gate.run_plans([plan], kind="changed") == 1

    records = list((evidence_root / "changed").glob("*.json"))
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
    evidence_root = tmp_path / "evidence"
    monkeypatch.setattr(gate, "_gate_evidence_root", lambda: evidence_root)
    plan = _plan(_cmd(["true"], "first"), _cmd(["true"], "second"))

    recorder = gate._start_recorder("full", [plan], "tree", gate._plan_fingerprint([plan]))

    assert recorder is not None
    record = json.loads(recorder.path.read_text())
    assert record["status"] == "running"
    assert record["finished_at"] is None
    assert [phase["status"] for phase in record["phases"]] == ["not_run", "not_run"]
    assert not list(recorder.path.parent.glob("*.tmp"))


def test_measurement_failure_does_not_replace_the_test_result(tmp_path, monkeypatch, capsys):
    marker = tmp_path / "phase-ran"
    monkeypatch.setattr(gate, "_gate_evidence_root", lambda: tmp_path / "evidence")
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")
    monkeypatch.setattr(gate, "_tree_fingerprint", lambda: "tree")
    monkeypatch.setattr(gate, "_write_run_record", _fail_write)

    result = gate.run_plans(
        [_plan(_cmd(["bash", "-c", f"touch {marker}"], "probe"))],
        kind="full",
    )

    assert result == 0
    assert marker.exists()
    assert capsys.readouterr().err.count("MEASUREMENT WARNING") == 1


def test_changed_measurement_warning_preserves_a_failing_result(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(gate, "_gate_evidence_root", lambda: tmp_path / "evidence")
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")
    monkeypatch.setattr(gate, "_tree_fingerprint", lambda: "tree")
    monkeypatch.setattr(gate, "_write_run_record", _fail_write)

    result = gate.run_plans(
        [_plan(_cmd(["bash", "-c", "exit 7"], "probe"))],
        kind="changed",
    )

    assert result == 1
    assert capsys.readouterr().err.count("MEASUREMENT WARNING") == 1


def test_identical_tree_and_plan_reuse_a_passing_run(tmp_path, monkeypatch, capsys):
    evidence_root = tmp_path / "evidence"
    marker = tmp_path / "ran"
    monkeypatch.setattr(gate, "_gate_evidence_root", lambda: evidence_root)
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")
    monkeypatch.setattr(gate, "_tree_fingerprint", lambda: "same-tree")
    plan = _plan(_cmd(["bash", "-c", f"test ! -e {marker} && touch {marker}"], "probe"))

    assert gate.run_plans([plan], reuse_passing=True) == 0
    assert gate.run_plans([plan], reuse_passing=True) == 0

    assert marker.exists()
    assert "Result: REUSED" in capsys.readouterr().out


def test_command_plan_change_invalidates_passing_evidence(tmp_path, monkeypatch):
    evidence_root = tmp_path / "evidence"
    marker = tmp_path / "ran"
    monkeypatch.setattr(gate, "_gate_evidence_root", lambda: evidence_root)
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")
    monkeypatch.setattr(gate, "_tree_fingerprint", lambda: "same-tree")
    first = _plan(_cmd(["bash", "-c", 'printf a >> "$1"', "_", str(marker)], "probe"))
    second = _plan(_cmd(["bash", "-c", 'printf b >> "$1"', "_", str(marker)], "probe"))

    assert gate.run_plans([first], reuse_passing=True) == 0
    assert gate.run_plans([second], reuse_passing=True) == 0

    assert marker.read_text() == "ab"


def test_failing_evidence_is_never_reused(tmp_path, monkeypatch, capsys):
    evidence_root = tmp_path / "evidence"
    marker = tmp_path / "first-attempt"
    monkeypatch.setattr(gate, "_gate_evidence_root", lambda: evidence_root)
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")
    monkeypatch.setattr(gate, "_tree_fingerprint", lambda: "same-tree")
    command = [
        "bash",
        "-c",
        'if test -e "$1"; then exit 0; else touch "$1"; exit 1; fi',
        "_",
        str(marker),
    ]
    plan = _plan(_cmd(command, "probe"))

    assert gate.run_plans([plan], reuse_passing=True) == 1
    capsys.readouterr()
    assert gate.run_plans([plan], reuse_passing=True) == 0

    assert "Result: REUSED" not in capsys.readouterr().out


def test_tree_content_change_invalidates_passing_evidence(tmp_path, monkeypatch):
    evidence_root = tmp_path / "evidence"
    marker = tmp_path / "ran"
    fingerprints = iter(["tree-a", "tree-b"])
    monkeypatch.setattr(gate, "_gate_evidence_root", lambda: evidence_root)
    monkeypatch.setattr(gate, "_run_artifact_root", lambda: tmp_path / "artifacts")
    monkeypatch.setattr(gate, "_tree_fingerprint", lambda: next(fingerprints))
    plan = _plan(_cmd(["bash", "-c", f"printf x >> {marker}"], "probe"))

    assert gate.run_plans([plan], reuse_passing=True) == 0
    assert gate.run_plans([plan], reuse_passing=True) == 0

    assert marker.read_text() == "xx"


def test_tree_fingerprint_tracks_content_not_git_staging_state(tmp_path, monkeypatch):
    repo = tmp_path / "repo"
    repo.mkdir()
    gate.subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
    gate.subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=repo, check=True)
    gate.subprocess.run(["git", "config", "user.name", "Test"], cwd=repo, check=True)
    tracked = repo / "tracked.txt"
    tracked.write_text("one\n")
    gate.subprocess.run(["git", "add", "tracked.txt"], cwd=repo, check=True)
    gate.subprocess.run(["git", "commit", "-qm", "base"], cwd=repo, check=True)
    monkeypatch.setattr(gate, "REPO_ROOT", repo)

    clean = gate._tree_fingerprint()
    tracked.write_text("two\n")
    unstaged = gate._tree_fingerprint()
    gate.subprocess.run(["git", "add", "tracked.txt"], cwd=repo, check=True)
    staged = gate._tree_fingerprint()
    gate.subprocess.run(["git", "commit", "-qm", "same content"], cwd=repo, check=True)
    committed = gate._tree_fingerprint()
    (repo / "untracked.txt").write_text("extra\n")
    untracked = gate._tree_fingerprint()

    assert clean != unstaged
    assert unstaged == staged == committed
    assert committed != untracked


def test_persisted_schema_contains_only_operational_facts():
    run = gate.GateRun(
        run_id="20260717T120000Z-1-abcdef12",
        kind="full",
        branch="test/branch",
        head="0123456789abcdef",
        worktree="/repo",
        tree_fingerprint="tree-fingerprint",
        plan_fingerprint="plan-fingerprint",
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
        "worktree",
        "tree_fingerprint",
        "plan_fingerprint",
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
    forbidden = {"argv", "output", "environment", "prompt", "diff"}
    assert forbidden.isdisjoint(json.dumps(record).lower().split('"'))


def test_full_and_host_gates_reject_reuse():
    for args in (["--all", "--reuse-passing"], ["--ui-host", "--reuse-passing"]):
        try:
            gate._parse_args(args)
        except SystemExit as exc:
            assert exc.code == 2
        else:
            raise AssertionError(f"reuse unexpectedly accepted for {args}")


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
