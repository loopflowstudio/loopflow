"""Tests for loopflow.pipeline module."""

from loopflow.pipeline import Pipeline


def test_pipeline_dataclass():
    """Pipeline holds name and task list."""
    p = Pipeline(name="ship", tasks=["implement", "review"])

    assert p.name == "ship"
    assert p.tasks == ["implement", "review"]
