"""Tests for flow DAG loading and execution."""

import tempfile
from pathlib import Path

from loopflow.lf.flow import topological_batches
from loopflow.lf.flows import (
    Flow,
    Fork,
    Step,
    build_step_dag,
    load_flow,
    save_flow,
)


def test_flow_from_dict_parses_steps():
    data = {
        "steps": [
            "design",
            {"step": "implement", "model": "codex", "direction": "architect"},
            {"step": "review", "after": "design"},
        ]
    }
    flow = Flow.from_dict("ship", data)
    assert flow.name == "ship"
    assert isinstance(flow.steps[0], Step)
    assert flow.steps[0].name == "design"
    assert flow.steps[1].model == "codex"
    assert flow.steps[1].direction == "architect"
    assert flow.steps[2].after == "design"


def test_build_step_dag_default_after():
    steps = [Step("a"), Step("b"), Step("c")]
    dag = build_step_dag(steps)
    assert dag.dependencies["a"] == set()
    assert dag.dependencies["b"] == {"a"}
    assert dag.dependencies["c"] == {"b"}


def test_build_step_dag_explicit_after():
    steps = [
        Step("design"),
        Step("impl-api", after="design"),
        Step("impl-ui", after="design"),
        Step("integrate", after=["impl-api", "impl-ui"]),
    ]
    dag = build_step_dag(steps)
    assert dag.dependencies["integrate"] == {"impl-api", "impl-ui"}


def test_topological_batches_parallel():
    steps = [
        Step("design"),
        Step("impl-api", after="design"),
        Step("impl-ui", after="design"),
        Step("integrate", after=["impl-api", "impl-ui"]),
    ]
    batches = topological_batches(build_step_dag(steps))
    assert [step.name for step in batches[0]] == ["design"]
    assert sorted(step.name for step in batches[1]) == ["impl-api", "impl-ui"]
    assert [step.name for step in batches[2]] == ["integrate"]


def test_load_flow_yaml():
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flows_dir = repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        (flows_dir / "simple.yaml").write_text("- implement\n- review\n")

        flow = load_flow("simple", repo)
        assert flow is not None
        assert [step.name for step in flow.steps if isinstance(step, Step)] == [
            "implement",
            "review",
        ]


def test_load_flow_with_fork():
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flows_dir = repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        (flows_dir / "race.yaml").write_text(
            """- fork:
    step: implement
    drafts:
      - direction: architect
      - direction: pragmatist
    synthesize:
      direction: reviewer
"""
        )

        flow = load_flow("race", repo)
        assert flow is not None
        assert isinstance(flow.steps[0], Fork)
        assert flow.steps[0].step == "implement"
        assert flow.steps[0].synthesize.direction == "reviewer"


def test_save_flow_roundtrip():
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flow = Flow(
            name="test",
            steps=[
                Step("design"),
                Step("implement", model="codex", direction="architect"),
            ],
        )

        path = save_flow(flow, repo)
        assert path.exists()

        loaded = load_flow("test", repo)
        assert loaded is not None
        assert isinstance(loaded.steps[0], Step)
        assert loaded.steps[1].model == "codex"
