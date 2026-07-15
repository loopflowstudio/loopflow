"""The full local gate is bounded and honest.

No phase can hang the gate indefinitely, a timeout is reported as a named
actionable failure, the required UI host gate never runs under --all, and a
simulated runner-bootstrap failure names the missing capability.
"""

import importlib.util
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


def test_hanging_phase_is_killed_at_its_budget(tmp_path, monkeypatch):
    # A phase that would sleep far past its budget must be killed promptly.
    monkeypatch.setitem(gate.PHASE_BUDGETS, "hang-probe", 1)
    started = time.monotonic()
    failure = gate._run_command(_cmd(["sleep", "60"], "hang-probe"), tmp_path)
    elapsed = time.monotonic() - started

    assert failure is not None
    assert "TIMEOUT" in failure and "hang-probe" in failure
    # Killed near its 1s budget plus the SIGTERM->SIGKILL grace, nowhere near 60s.
    assert elapsed < 20, f"phase took {elapsed:.1f}s; the budget did not bound it"


def test_child_process_dies_with_the_group(tmp_path, monkeypatch):
    # A phase that backgrounds a long sleeper must not leave the sleeper alive:
    # the group-kill reaps children too. The marker file would be written only
    # if the child outlived the kill.
    monkeypatch.setitem(gate.PHASE_BUDGETS, "group-probe", 1)
    marker = tmp_path / "child-survived"
    script = f"sleep 30 && touch {marker} & wait"
    gate._run_command(_cmd(["bash", "-c", script], "group-probe"), tmp_path)
    time.sleep(3)
    assert not marker.exists(), "backgrounded child outlived the group-kill"


def test_failing_phase_reports_actionable_failure(tmp_path):
    failure = gate._run_command(_cmd(["bash", "-c", "exit 3"], "red-probe"), tmp_path)
    assert failure is not None
    assert "FAILED" in failure and "red-probe" in failure
    assert "exit 3" in failure


def test_passing_phase_returns_none(tmp_path):
    assert gate._run_command(_cmd(["bash", "-c", "exit 0"], "ok-probe"), tmp_path) is None


def test_missing_tool_is_named_not_a_traceback(tmp_path):
    failure = gate._run_command(_cmd(["definitely-not-a-real-binary-xyz"], "tool-probe"), tmp_path)
    assert failure is not None
    assert "MISSING TOOL" in failure


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


def test_loopflow_summary_says_it_does_not_run_hosted_ui():
    loopflow = next(s for s in gate.SUITES if s.name == "loopflow")
    assert loopflow.proves is not None
    assert "NOT run here" in loopflow.proves
