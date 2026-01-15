"""Tests for pipeline DAG loading and execution."""

import tempfile
from pathlib import Path

from loopflow.lfd.pipelines import (
    PipelineDef,
    PipelineStep,
    StepConfig,
    load_pipeline,
    resolve_pipeline,
)


def test_pipeline_step_from_string():
    step = PipelineStep.from_dict("implement")
    assert step.task == "implement"
    assert step.pipeline is None
    assert step.parallel is None


def test_pipeline_step_from_dict():
    step = PipelineStep.from_dict({
        "task": "review",
        "config": {"model": "claude:opus"},
    })
    assert step.task == "review"
    assert step.config is not None
    assert step.config.model == "claude:opus"


def test_pipeline_step_with_parallel():
    step = PipelineStep.from_dict({
        "parallel": [
            {"task": "test"},
            {"task": "lint"},
        ]
    })
    assert step.parallel is not None
    assert len(step.parallel) == 2
    assert step.parallel[0].task == "test"
    assert step.parallel[1].task == "lint"


def test_pipeline_step_serialization():
    step = PipelineStep(
        task="implement",
        config=StepConfig(model="claude:opus"),
    )
    data = step.to_dict()
    restored = PipelineStep.from_dict(data)
    assert restored.task == "implement"
    assert restored.config.model == "claude:opus"


def test_pipeline_def_from_dict():
    data = {
        "steps": [
            "design",
            "implement",
            {"parallel": [{"task": "test"}, {"task": "lint"}]},
            "land",
        ]
    }
    pipeline = PipelineDef.from_dict("ship", data)
    assert pipeline.name == "ship"
    assert len(pipeline.steps) == 4
    assert pipeline.steps[0].task == "design"
    assert pipeline.steps[2].parallel is not None


def test_load_pipeline():
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        pipelines_dir = repo / ".lf" / "pipelines"
        pipelines_dir.mkdir(parents=True)

        (pipelines_dir / "ship.yaml").write_text("""
steps:
  - design
  - implement
  - parallel:
      - test
      - lint
  - land
""")

        pipeline = load_pipeline("ship", repo)
        assert pipeline is not None
        assert pipeline.name == "ship"
        assert len(pipeline.steps) == 4


def test_load_pipeline_not_found():
    with tempfile.TemporaryDirectory() as tmpdir:
        pipeline = load_pipeline("nonexistent", Path(tmpdir))
        assert pipeline is None


def test_resolve_pipeline_sequential():
    pipeline = PipelineDef(
        name="simple",
        steps=[
            PipelineStep(task="a"),
            PipelineStep(task="b"),
            PipelineStep(task="c"),
        ],
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        resolved = resolve_pipeline(pipeline, Path(tmpdir))

    assert len(resolved) == 3
    assert resolved[0].task == "a"
    assert resolved[0].parallel_group is None
    assert resolved[1].task == "b"
    assert resolved[2].task == "c"


def test_resolve_pipeline_with_parallel():
    pipeline = PipelineDef(
        name="parallel",
        steps=[
            PipelineStep(task="a"),
            PipelineStep(parallel=[
                PipelineStep(task="b"),
                PipelineStep(task="c"),
            ]),
            PipelineStep(task="d"),
        ],
    )

    with tempfile.TemporaryDirectory() as tmpdir:
        resolved = resolve_pipeline(pipeline, Path(tmpdir))

    assert len(resolved) == 4
    assert resolved[0].task == "a"
    assert resolved[0].parallel_group is None

    # b and c should be in the same parallel group
    assert resolved[1].task == "b"
    assert resolved[1].parallel_group == 0
    assert resolved[2].task == "c"
    assert resolved[2].parallel_group == 0

    assert resolved[3].task == "d"
    assert resolved[3].parallel_group is None


def test_resolve_pipeline_nested():
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        pipelines_dir = repo / ".lf" / "pipelines"
        pipelines_dir.mkdir(parents=True)

        # Create a nested pipeline
        (pipelines_dir / "inner.yaml").write_text("""
steps:
  - test
  - lint
""")

        pipeline = PipelineDef(
            name="outer",
            steps=[
                PipelineStep(task="implement"),
                PipelineStep(pipeline="inner"),
                PipelineStep(task="land"),
            ],
        )

        resolved = resolve_pipeline(pipeline, repo)

        assert len(resolved) == 4
        assert resolved[0].task == "implement"
        assert resolved[1].task == "test"
        assert resolved[2].task == "lint"
        assert resolved[3].task == "land"


def test_step_config_voice():
    """StepConfig supports voice field."""
    config = StepConfig(model="claude:opus", voice="architect")
    data = config.to_dict()

    assert data["model"] == "claude:opus"
    assert data["voice"] == "architect"

    restored = StepConfig.from_dict(data)
    assert restored.voice == "architect"


def test_step_config_context():
    """StepConfig supports context field."""
    config = StepConfig(context=["src/schema.py", "docs/api.md"])
    data = config.to_dict()

    assert data["context"] == ["src/schema.py", "docs/api.md"]

    restored = StepConfig.from_dict(data)
    assert restored.context == ["src/schema.py", "docs/api.md"]


def test_pipeline_step_with_full_config():
    """PipelineStep preserves voice and context in config."""
    step = PipelineStep.from_dict({
        "task": "implement",
        "config": {
            "model": "claude:opus",
            "voice": "architect",
            "context": ["src/models.py"],
        }
    })

    assert step.task == "implement"
    assert step.config is not None
    assert step.config.model == "claude:opus"
    assert step.config.voice == "architect"
    assert step.config.context == ["src/models.py"]

    # Round-trip
    data = step.to_dict()
    restored = PipelineStep.from_dict(data)
    assert restored.config.voice == "architect"
    assert restored.config.context == ["src/models.py"]


def test_load_pipeline_with_config():
    """Load pipeline with per-step config from YAML."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = Path(tmpdir)
        pipelines_dir = repo / ".lf" / "pipelines"
        pipelines_dir.mkdir(parents=True)

        (pipelines_dir / "ship.yaml").write_text("""
steps:
  - task: design
  - task: implement
    config:
      model: claude:opus
      voice: architect
      context:
        - src/schema.py
  - task: review
""")

        pipeline = load_pipeline("ship", repo)
        assert pipeline is not None
        assert len(pipeline.steps) == 3

        # First step has no config
        assert pipeline.steps[0].task == "design"
        assert pipeline.steps[0].config is None

        # Second step has full config
        assert pipeline.steps[1].task == "implement"
        assert pipeline.steps[1].config is not None
        assert pipeline.steps[1].config.model == "claude:opus"
        assert pipeline.steps[1].config.voice == "architect"
        assert pipeline.steps[1].config.context == ["src/schema.py"]

        # Third step has no config
        assert pipeline.steps[2].task == "review"
        assert pipeline.steps[2].config is None
