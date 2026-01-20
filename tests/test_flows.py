"""Tests for flow DAG loading and execution."""

import tempfile
from pathlib import Path

from loopflow.lf.flows import (
    FlowDef,
    FlowStep,
    RaceConfig,
    StepConfig,
    load_flow,
    save_flow,
    resolve_flow,
)
from loopflow.lf.flow import _count_logical_steps


def test_flow_step_from_string():
    step = FlowStep.from_dict("implement")
    assert step.step == "implement"
    assert step.flow is None
    assert step.parallel is None


def test_flow_step_from_dict():
    step = FlowStep.from_dict({
        "step": "review",
        "config": {"model": "claude:opus"},
    })
    assert step.step == "review"
    assert step.config is not None
    assert step.config.model == "claude:opus"


def test_flow_step_with_parallel():
    step = FlowStep.from_dict({
        "parallel": [
            {"step": "test"},
            {"step": "lint"},
        ]
    })
    assert step.parallel is not None
    assert len(step.parallel) == 2
    assert step.parallel[0].step == "test"
    assert step.parallel[1].step == "lint"


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
            {"parallel": [{"step": "test"}, {"step": "lint"}]},
            "land",
        ]
    }
    flow = FlowDef.from_dict("ship", data)
    assert flow.name == "ship"
    assert len(flow.steps) == 4
    assert flow.steps[0].step == "design"
    assert flow.steps[2].parallel is not None


def test_load_flow():
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flows_dir = repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        (flows_dir / "ship.yaml").write_text("""
steps:
  - design
  - implement
  - parallel:
      - test
      - lint
  - land
""")

        flow = load_flow("ship", repo)
        assert flow is not None
        assert flow.name == "ship"
        assert len(flow.steps) == 4


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


def test_resolve_flow_with_parallel():
    flow = FlowDef(
        name="parallel",
        steps=[
            FlowStep(step="a"),
            FlowStep(parallel=[
                FlowStep(step="b"),
                FlowStep(step="c"),
            ]),
            FlowStep(step="d"),
        ],
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        resolved = resolve_flow(flow, Path(tmpdir))

    assert len(resolved) == 4
    assert resolved[0].step == "a"
    assert resolved[0].parallel_group is None

    # b and c should be in the same parallel group
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
        (flows_dir / "inner.yaml").write_text("""
steps:
  - test
  - lint
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
    step = FlowStep.from_dict({
        "step": "implement",
        "config": {
            "model": "claude:opus",
            "voice": "architect",
            "context": ["src/models.py"],
        }
    })

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

        (flows_dir / "ship.yaml").write_text("""
steps:
  - step: design
  - step: implement
    config:
      model: claude:opus
      voice: architect
      context:
        - src/schema.py
  - step: review
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
                FlowStep(step="implement", config=StepConfig(
                    model="claude:opus",
                    voice=["architect"],
                    context=["src/main.py"],
                )),
                FlowStep(step="review"),
            ],
        )

        path = save_flow(flow, repo)
        assert path.exists()
        assert path.name == "test.yaml"

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


def test_count_logical_steps_with_parallel():
    """Parallel groups count as 1 logical step."""
    flow = FlowDef(
        name="parallel",
        steps=[
            FlowStep(step="a"),
            FlowStep(parallel=[
                FlowStep(step="b"),
                FlowStep(step="c"),
            ]),
            FlowStep(step="d"),
        ],
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        resolved = resolve_flow(flow, Path(tmpdir))
        count = _count_logical_steps(resolved)

    # a, parallel(b+c), d = 3 logical steps
    assert count == 3


def test_count_logical_steps_multiple_parallel_groups():
    """Multiple parallel groups each count as 1."""
    flow = FlowDef(
        name="multi-parallel",
        steps=[
            FlowStep(parallel=[
                FlowStep(step="a"),
                FlowStep(step="b"),
            ]),
            FlowStep(step="c"),
            FlowStep(parallel=[
                FlowStep(step="d"),
                FlowStep(step="e"),
            ]),
        ],
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        resolved = resolve_flow(flow, Path(tmpdir))
        count = _count_logical_steps(resolved)

    # parallel(a+b), c, parallel(d+e) = 3 logical steps
    assert count == 3


def test_load_flow_with_parallel():
    """Load flow with parallel steps from YAML."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flows_dir = repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        (flows_dir / "verify.yaml").write_text("""
steps:
  - implement
  - parallel:
      - step: test
      - step: lint
  - commit
""")

        flow = load_flow("verify", repo)
        assert flow is not None
        assert len(flow.steps) == 3

        # First is sequential
        assert flow.steps[0].step == "implement"

        # Second is parallel group
        assert flow.steps[1].parallel is not None
        assert len(flow.steps[1].parallel) == 2
        assert flow.steps[1].parallel[0].step == "test"
        assert flow.steps[1].parallel[1].step == "lint"

        # Third is sequential
        assert flow.steps[2].step == "commit"

        # Resolve and check groups
        resolved = resolve_flow(flow, repo)
        assert len(resolved) == 4  # implement, test, lint, commit

        assert resolved[0].parallel_group is None
        assert resolved[1].parallel_group == 0
        assert resolved[2].parallel_group == 0
        assert resolved[3].parallel_group is None


# Race configuration tests

def test_race_config_from_list():
    """RaceConfig parses simple list of models."""
    race = RaceConfig.from_dict(["claude:opus", "codex:o3"])
    assert race.models == ["claude:opus", "codex:o3"]
    assert race.judge == "compare"


def test_race_config_from_dict():
    """RaceConfig parses full dict with custom judge."""
    race = RaceConfig.from_dict({
        "models": ["claude:opus", "codex:o3"],
        "judge": "custom-judge",
    })
    assert race.models == ["claude:opus", "codex:o3"]
    assert race.judge == "custom-judge"


def test_race_config_from_model_objects():
    """RaceConfig parses list of model objects."""
    race = RaceConfig.from_dict([
        {"model": "claude:opus"},
        {"model": "codex:o3"},
    ])
    assert race.models == ["claude:opus", "codex:o3"]


def test_race_config_serialization():
    """RaceConfig round-trips through to_dict."""
    race = RaceConfig(models=["claude:opus", "codex:o3"], judge="custom")
    data = race.to_dict()
    restored = RaceConfig.from_dict(data)
    assert restored.models == ["claude:opus", "codex:o3"]
    assert restored.judge == "custom"


def test_flow_step_with_race():
    """FlowStep parses race configuration."""
    step = FlowStep.from_dict({
        "step": "implement",
        "race": ["claude:opus", "codex:o3"],
    })
    assert step.step == "implement"
    assert step.race is not None
    assert step.race.models == ["claude:opus", "codex:o3"]


def test_flow_step_race_serialization():
    """FlowStep with race round-trips."""
    step = FlowStep(
        step="implement",
        race=RaceConfig(models=["claude:opus", "codex:o3"]),
    )
    data = step.to_dict()
    restored = FlowStep.from_dict(data)
    assert restored.race is not None
    assert restored.race.models == ["claude:opus", "codex:o3"]


def test_load_flow_with_race():
    """Load flow with race step from YAML."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        flows_dir = repo / ".lf" / "flows"
        flows_dir.mkdir(parents=True)

        (flows_dir / "raceship.yaml").write_text("""
steps:
  - design
  - step: implement
    race:
      - claude:opus
      - codex:o3
  - review
""")

        flow = load_flow("raceship", repo)
        assert flow is not None
        assert len(flow.steps) == 3

        # First step is simple
        assert flow.steps[0].step == "design"
        assert flow.steps[0].race is None

        # Second step has race
        assert flow.steps[1].step == "implement"
        assert flow.steps[1].race is not None
        assert flow.steps[1].race.models == ["claude:opus", "codex:o3"]

        # Third step is simple
        assert flow.steps[2].step == "review"
        assert flow.steps[2].race is None


def test_resolve_flow_with_race():
    """Resolved steps preserve race configuration."""
    flow = FlowDef(
        name="raceship",
        steps=[
            FlowStep(step="design"),
            FlowStep(step="implement", race=RaceConfig(models=["claude", "codex"])),
            FlowStep(step="review"),
        ],
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        resolved = resolve_flow(flow, Path(tmpdir))

    assert len(resolved) == 3
    assert resolved[0].race is None
    assert resolved[1].race is not None
    assert resolved[1].race.models == ["claude", "codex"]
    assert resolved[2].race is None
