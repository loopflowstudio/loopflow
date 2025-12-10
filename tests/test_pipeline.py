"""Tests for loopflow.pipeline module."""

from loopflow.pipeline import Pipeline


def test_pipeline_dataclass():
    """Pipeline holds name and task list."""
    p = Pipeline(name="ship", tasks=["implement", "review"])

    assert p.name == "ship"
    assert p.tasks == ["implement", "review"]


def test_pipeline_with_push_pr():
    """Pipeline can override push/pr settings."""
    p = Pipeline(name="ship", tasks=["implement"], push=True, pr=True)

    assert p.push is True
    assert p.pr is True


def test_pipeline_defaults_none():
    """Pipeline push/pr default to None."""
    p = Pipeline(name="ship", tasks=["implement"])

    assert p.push is None
    assert p.pr is None
