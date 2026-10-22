from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

SCRIPT = (
    Path(__file__).resolve().parents[2] / "scripts/benchmarks/desktop-performance/agent_input.py"
)
spec = importlib.util.spec_from_file_location("agent_input", SCRIPT)
profile = importlib.util.module_from_spec(spec)
spec.loader.exec_module(profile)


def _trial(directory: Path, provider: str = "codex") -> None:
    directory.mkdir(exist_ok=True)
    metadata = {key: "fixture" for key in profile.REQUIRED}
    metadata.update(provider=provider, case="resume", task="task-owned", session="session-owned")
    (directory / "metadata.json").write_text(json.dumps(metadata))
    (directory / "capture.json").write_text(
        json.dumps({"start": 9, "end": 15, "interrupted": False, "pids": [123]})
    )
    evidence = directory / "native-capture.png"
    evidence.write_bytes(b"fixture; no native proof")
    rows = [
        {"phase": phase, "wall": wall, "evidence": str(evidence)}
        for phase, wall in zip(profile.PHASES[:3], (11, 12, 14))
    ]
    (directory / "markers.ndjson").write_text("".join(json.dumps(row) + "\n" for row in rows))
    events = [
        "phase=task_click task=task-owned wall=10",
        "phase=key_received session=session-owned wall=13",
        "phase=key_draw session=session-owned wall=13.5",
    ]
    (directory / "signposts.ndjson").write_text(
        "".join(json.dumps({"eventMessage": event}) + "\n" for event in events)
    )


def test_surface_and_draw_alone_never_establish_provider_readiness(tmp_path: Path) -> None:
    _trial(tmp_path)
    (tmp_path / "markers.ndjson").unlink()
    report = profile.summarize(tmp_path)
    assert report["click_to_observed_response_ms"] is None
    assert report["status"] == "unproved"
    assert report["cpu_unavailable"] is not None


def test_reviewable_endpoint_requires_exact_session_and_ordered_evidence(tmp_path: Path) -> None:
    _trial(tmp_path)
    assert profile.summarize(tmp_path)["click_to_observed_response_ms"] == 4000
    signposts = tmp_path / "signposts.ndjson"
    signposts.write_text(signposts.read_text().replace("session-owned", "session-foreign"))
    assert profile.summarize(tmp_path)["click_to_observed_response_ms"] is None


def test_failed_or_interrupted_trials_do_not_become_successes(tmp_path: Path) -> None:
    _trial(tmp_path)
    (tmp_path / "capture.json").write_text(
        json.dumps({"start": 10, "end": 13, "interrupted": True})
    )
    assert profile.summarize(tmp_path)["click_to_observed_response_ms"] is None
    (tmp_path / "capture.json").write_text(
        json.dumps({"start": 9, "end": 15, "interrupted": False})
    )
    profile.mark(tmp_path, "failed", tmp_path / "native-capture.png")
    assert profile.summarize(tmp_path)["status"] == "failed"


@pytest.mark.parametrize("phase", ["opening_failed", "provider_spawn_failed"])
def test_launch_failure_cannot_supply_an_endpoint_or_comparison(tmp_path: Path, phase: str) -> None:
    before, after = tmp_path / "before", tmp_path / "after"
    _trial(before)
    _trial(after)
    (after / "lf-events.ndjson").write_text(
        json.dumps({"phase": phase, "session": "session-owned", "wall": 11}) + "\n"
    )
    report = profile.summarize(after)
    assert report["status"] == "failed"
    assert report["click_to_observed_response_ms"] is None
    assert profile.compare(before, after)["delta_ms"] is None


def test_provider_and_case_comparisons_stay_separate(tmp_path: Path) -> None:
    _trial(tmp_path / "before")
    _trial(tmp_path / "after", provider="claude")
    with pytest.raises(ValueError, match="Incomparable trials"):
        profile.compare(tmp_path / "before", tmp_path / "after")


def test_flames_resolve_shared_stacks_and_exclude_other_processes(tmp_path: Path) -> None:
    xml = tmp_path / "cpu.xml"
    xml.write_text("""<trace-query-result><node><row>
      <thread-state id="6">Running</thread-state>
      <thread id="1"><process id="2" fmt="lf (123)"><pid id="3">123</pid></process></thread>
      <backtrace id="4"><frame id="5" name="leaf &amp; work"/><frame name="main"/></backtrace>
      </row><row><thread-state ref="6"/><thread ref="1"/><backtrace ref="4"/></row>
      <row><thread-state>Running</thread-state><process fmt="other"><pid>456</pid></process>
      <backtrace><frame name="unrelated"/></backtrace></row>
      </node></trace-query-result>""")
    stacks = profile.folded_stacks(xml, {123})
    assert stacks == {"lf (123);main;leaf & work": 2}
    output = tmp_path / "cpu.svg"
    profile.flamegraph(stacks, output)
    assert "leaf &amp; work" in output.read_text()
    assert "unrelated" not in output.read_text()
    profile.flamegraph({}, output)
    assert not output.exists()


def test_lf_spans_and_wall_events_share_timeline(tmp_path: Path) -> None:
    _trial(tmp_path)
    (tmp_path / "lf-events.ndjson").write_text(
        json.dumps({"phase": "provider_spawn_returned", "session": "session-owned", "wall": 11.5})
        + "\n"
    )
    with (tmp_path / "signposts.ndjson").open("a") as output:
        output.write(
            json.dumps(
                {
                    "signpostName": "lf",
                    "signpostType": "begin",
                    "signpostID": 4,
                    "eventMessage": "trace=owned session ensure",
                    "timestamp": "1970-01-01 00:00:10.500000+0000",
                }
            )
            + "\n"
        )
    phases = [row["phase"] for row in profile.events(tmp_path)]
    assert phases[:4] == ["task_click", "lf_begin", "input_visible", "provider_spawn_returned"]


def test_cpu_snapshots_report_deltas_without_counting_preexisting_cpu(tmp_path: Path) -> None:
    _trial(tmp_path)
    (tmp_path / "processes.ndjson").write_text(
        json.dumps({"processes": [{"pid": 123, "cpu": "01:00.20"}]})
        + "\n"
        + json.dumps({"processes": [{"pid": 123, "cpu": "01:01.70"}]})
        + "\n"
    )
    assert profile.summarize(tmp_path)["observed_cpu_seconds_by_pid"] == {123: 1.5}
