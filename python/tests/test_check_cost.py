import json
from pathlib import Path

import pytest

from scripts.check_cost import _is_check, compare, summarize


def test_commands_distinguish_execution_from_reading_test_names():
    assert _is_check(["/bin/zsh -c 'cd swift && swift test --filter DesktopHeadlessTests'"])
    assert _is_check(["uv", "run", "pytest", "python/tests/test_check_cost.py"])
    assert _is_check(["cargo", "nextest", "run"])
    assert _is_check(["uv", "run", "--project", "website", "--extra", "test", "pytest"])
    assert _is_check(["/bin/zsh", "-c", "set -e\nnice -n 10 cargo test"])
    assert not _is_check(["rg", "pytest", "TESTING.md"])
    assert not _is_check(["rg", "cargo test", "TESTING.md"])
    assert not _is_check(["cat", "scripts/test.py"])


def test_report_counts_overlapping_checks_once_and_exposes_missing_runs(tmp_path: Path):
    repo = tmp_path / "repo"
    repo.mkdir()
    runs = tmp_path / "runs"
    directory = runs / "ab" / "run-one"
    directory.mkdir(parents=True)
    manifest = {"created_at": "2026-09-30T10:00:00Z", "repo": str(repo), "skill": "implement"}
    (directory / "manifest.json").write_text(json.dumps(manifest))
    (directory / "terminal.json").write_text(json.dumps({"ended_at": "2026-09-30T10:01:00Z"}))
    events = []
    for identity, start, stop in [("one", 10, 30), ("two", 20, 40)]:
        for kind, second in [("item_started", start), ("item_completed", stop)]:
            events.append(
                {
                    "type": "conversation",
                    "observed_at": f"2026-09-30T10:00:{second:02d}Z",
                    "event": {
                        "type": kind,
                        "item": {
                            "type": "command",
                            "id": identity,
                            "command": ["swift", "test"],
                            "exit_code": 0,
                        },
                    },
                }
            )
    # Duplicate completion records cannot inflate counts or duration.
    events.append(events[-1])
    (directory / "events.jsonl").write_text("\n".join(json.dumps(e) for e in events))
    pending = directory.parent / "run-two"
    pending.mkdir()
    (pending / "manifest.json").write_text(json.dumps(manifest))
    report = summarize(runs, repo, "2026-09-30T00:00:00Z", "2026-10-01T00:00:00Z")
    step = report["steps"]["implement"]
    assert step["run_seconds"] == 60
    assert step["check_seconds"] == 30
    assert step["check_commands"] == 2
    assert step["settled_runs"] == 1
    assert step["unsettled_runs"] == 1
    assert step["missing_context_runs"] == 1


@pytest.mark.parametrize("gap", ["runs_without_command_records", "commands_without_start"])
def test_comparison_does_not_call_missing_commands_an_improvement(gap: str):
    baseline = {
        "steps": {"implement": {"settled_runs": 2, "run_seconds": 100, "check_seconds": 30}}
    }
    current = {"steps": {"implement": {"settled_runs": 1, "run_seconds": 100, "check_seconds": 10}}}
    assert compare(baseline, current)["implement"]["change_percentage_points"] == -20
    current["steps"]["implement"][gap] = 1
    assert compare(baseline, current) == {}
