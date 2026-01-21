"""Tests for flow DAG loading and execution."""

import tempfile
from pathlib import Path

from loopflow.lf.flow import _count_logical_steps
from loopflow.lf.flows import FlowDef, FlowStep, load_flow, resolve_flow, save_flow
from loopflow.lf.frontmatter import StepConfig


def test_flow_step_from_string():
    step = FlowStep.from_dict("implement")
    assert step.step == "implement"
    assert step.flow is None
    assert step.fork is None


def test_flow_step_from_dict():
    step = FlowStep.from_dict(
        {
            "step": "review",
            "config": {"model": "claude:opus"},
        }
    )
    assert step.step == "review"
    assert step.config is not None
    assert step.config.model == "claude:opus"


def test_flow_step_with_fork():
    step = FlowStep.from_dict(
        {
            "fork": [
                {"step": "test"},
                {"step": "lint"},
            ]
        }
    )
    assert step.fork is not None
    assert len(step.fork) == 2
    assert step.fork[0].step == "test"
    assert step.fork[1].step == "lint"


def test_flow_step_serialization():
    step = FlowStep(
        step="implement",
        config=StepConfig(model="claude:opus"),
    )
    data = step.to_dict()
    restored = FlowStep.from_dict(data)
    assert restored.step == "implement"
    assert restored.config.model == "claude:opus"


def test_flow_def_from_dict():
    data = {
        "steps": [
            "design",
            "implement",
            {"fork": [{"step": "test"}, {"step": "lint"}]},
            "land",
        ]
    }
    flow = FlowDef.from_dict("ship", data)
    assert flow.name == "ship"
    assert len(flow.steps) == 4
    assert flow.steps[0].step == "design"
    assert flow.steps[2].fork is not None


def test_load_flow():
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flows_dir = repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        (flows_dir / "ship.py").write_text("""
def flow():
    return {
        "steps": [
            "design",
            "implement",
            {"fork": ["test", "lint"]},
            "land",
        ]
    }
""")

        flow = load_flow("ship", repo)
        assert flow is not None
        assert flow.name == "ship"
        assert len(flow.steps) == 4


def test_load_flow_with_flow_list_helper():
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flows_dir = repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        (flows_dir / "simple.py").write_text("""
def flow():
    return Flow("implement", "review")
""")

        flow = load_flow("simple", repo)
        assert flow is not None
        assert [step.step for step in flow.steps] == ["implement", "review"]


def test_load_flow_with_named_constant():
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flows_dir = repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        (flows_dir / "ship.py").write_text("""
SHIP = Flow("design", "implement")
""")

        flow = load_flow("ship", repo)
        assert flow is not None
        assert [step.step for step in flow.steps] == ["design", "implement"]


def test_load_flow_not_found():
    with tempfile.TemporaryDirectory() as tmpdir:
        flow = load_flow("nonexistent", Path(tmpdir))
        assert flow is None


def test_resolve_flow_sequential():
    flow = FlowDef(
        name="simple",
        steps=[
            FlowStep(step="a"),
            FlowStep(step="b"),
            FlowStep(step="c"),
        ],
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        resolved = resolve_flow(flow, Path(tmpdir))

    assert len(resolved) == 3
    assert resolved[0].step == "a"
    assert resolved[0].parallel_group is None
    assert resolved[1].step == "b"
    assert resolved[2].step == "c"


def test_resolve_flow_with_fork():
    flow = FlowDef(
        name="parallel",
        steps=[
            FlowStep(step="a"),
            FlowStep(
                fork=[
                    FlowStep(step="b"),
                    FlowStep(step="c"),
                ]
            ),
            FlowStep(step="d"),
        ],
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        resolved = resolve_flow(flow, Path(tmpdir))

    assert len(resolved) == 4
    assert resolved[0].step == "a"
    assert resolved[0].parallel_group is None

    # b and c should be in the same fork group
    assert resolved[1].step == "b"
    assert resolved[1].parallel_group == 0
    assert resolved[2].step == "c"
    assert resolved[2].parallel_group == 0

    assert resolved[3].step == "d"
    assert resolved[3].parallel_group is None


def test_resolve_flow_nested():
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flows_dir = repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        # Create a nested flow
        (flows_dir / "inner.py").write_text("""
def flow():
    return {"steps": ["test", "lint"]}
""")

        flow = FlowDef(
            name="outer",
            steps=[
                FlowStep(step="implement"),
                FlowStep(flow="inner"),
                FlowStep(step="land"),
            ],
        )

        resolved = resolve_flow(flow, repo)

        assert len(resolved) == 4
        assert resolved[0].step == "implement"
        assert resolved[1].step == "test"
        assert resolved[2].step == "lint"
        assert resolved[3].step == "land"


def test_step_config_voice():
    """StepConfig supports voice field."""
    config = StepConfig(model="claude:opus", voice=["architect"])
    data = config.to_dict()

    assert data["model"] == "claude:opus"
    assert data["voice"] == ["architect"]

    restored = StepConfig.from_dict(data)
    assert restored.voice == ["architect"]


def test_step_config_context():
    """StepConfig supports context field."""
    config = StepConfig(context=["src/schema.py", "docs/api.md"])
    data = config.to_dict()

    assert data["context"] == ["src/schema.py", "docs/api.md"]

    restored = StepConfig.from_dict(data)
    assert restored.context == ["src/schema.py", "docs/api.md"]


def test_flow_step_with_full_config():
    """FlowStep preserves voice and context in config."""
    step = FlowStep.from_dict(
        {
            "step": "implement",
            "config": {
                "model": "claude:opus",
                "voice": "architect",
                "context": ["src/models.py"],
            },
        }
    )

    assert step.step == "implement"
    assert step.config is not None
    assert step.config.model == "claude:opus"
    assert step.config.voice == ["architect"]
    assert step.config.context == ["src/models.py"]

    # Round-trip
    data = step.to_dict()
    restored = FlowStep.from_dict(data)
    assert restored.config.voice == ["architect"]
    assert restored.config.context == ["src/models.py"]


def test_load_flow_with_config():
    """Load flow with per-step config from YAML."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flows_dir = repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        (flows_dir / "ship.py").write_text("""
def flow():
    return {
        "steps": [
            "design",
            {
                "step": "implement",
                "config": {
                    "model": "claude:opus",
                    "voice": "architect",
                    "context": ["src/schema.py"],
                },
            },
            "review",
        ]
    }
""")

        flow = load_flow("ship", repo)
        assert flow is not None
        assert len(flow.steps) == 3

        # First step has no config
        assert flow.steps[0].step == "design"
        assert flow.steps[0].config is None

        # Second step has full config
        assert flow.steps[1].step == "implement"
        assert flow.steps[1].config is not None
        assert flow.steps[1].config.model == "claude:opus"
        assert flow.steps[1].config.voice == ["architect"]
        assert flow.steps[1].config.context == ["src/schema.py"]

        # Third step has no config
        assert flow.steps[2].step == "review"
        assert flow.steps[2].config is None


def test_save_flow():
    """Save flow writes correct YAML."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)

        flow = FlowDef(
            name="test",
            steps=[
                FlowStep(step="design"),
                FlowStep(
                    step="implement",
                    config=StepConfig(
                        model="claude:opus",
                        voice=["architect"],
                        context=["src/main.py"],
                    ),
                ),
                FlowStep(step="review"),
            ],
        )

        path = save_flow(flow, repo)
        assert path.exists()
        assert path.name == "test.py"

        # Load it back and verify
        loaded = load_flow("test", repo)
        assert loaded is not None
        assert len(loaded.steps) == 3
        assert loaded.steps[1].config.model == "claude:opus"
        assert loaded.steps[1].config.voice == ["architect"]
        assert loaded.steps[1].config.context == ["src/main.py"]


def test_count_logical_steps_sequential():
    """Sequential steps each count as 1."""
    flow = FlowDef(
        name="simple",
        steps=[
            FlowStep(step="a"),
            FlowStep(step="b"),
            FlowStep(step="c"),
        ],
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        resolved = resolve_flow(flow, Path(tmpdir))
        count = _count_logical_steps(resolved)

    assert count == 3


def test_count_logical_steps_with_fork():
    """Fork groups count as 1 logical step."""
    flow = FlowDef(
        name="parallel",
        steps=[
            FlowStep(step="a"),
            FlowStep(
                fork=[
                    FlowStep(step="b"),
                    FlowStep(step="c"),
                ]
            ),
            FlowStep(step="d"),
        ],
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        resolved = resolve_flow(flow, Path(tmpdir))
        count = _count_logical_steps(resolved)

    # a, fork(b+c), d = 3 logical steps
    assert count == 3


def test_count_logical_steps_multiple_fork_groups():
    """Multiple fork groups each count as 1."""
    flow = FlowDef(
        name="multi-parallel",
        steps=[
            FlowStep(
                fork=[
                    FlowStep(step="a"),
                    FlowStep(step="b"),
                ]
            ),
            FlowStep(step="c"),
            FlowStep(
                fork=[
                    FlowStep(step="d"),
                    FlowStep(step="e"),
                ]
            ),
        ],
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        resolved = resolve_flow(flow, Path(tmpdir))
        count = _count_logical_steps(resolved)

    # fork(a+b), c, fork(d+e) = 3 logical steps
    assert count == 3


def test_load_flow_with_fork():
    """Load flow with fork steps from YAML."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flows_dir = repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        (flows_dir / "verify.py").write_text("""
def flow():
    return {
            "steps": [
                "implement",
                {"fork": [{"step": "test"}, {"step": "lint"}]},
                "commit",
            ]
        }
""")

        flow = load_flow("verify", repo)
        assert flow is not None
        assert len(flow.steps) == 3

        # First is sequential
        assert flow.steps[0].step == "implement"

        # Second is fork group
        assert flow.steps[1].fork is not None
        assert len(flow.steps[1].fork) == 2
        assert flow.steps[1].fork[0].step == "test"
        assert flow.steps[1].fork[1].step == "lint"

        # Third is sequential
        assert flow.steps[2].step == "commit"

        # Resolve and check groups
        resolved = resolve_flow(flow, repo)
        assert len(resolved) == 4  # implement, test, lint, commit

        assert resolved[0].parallel_group is None
        assert resolved[1].parallel_group == 0
        assert resolved[2].parallel_group == 0
        assert resolved[3].parallel_group is None
