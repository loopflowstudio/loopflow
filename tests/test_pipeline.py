"""Tests for loopflow.config module - PipelineConfig."""

from loopflow.lf.config import PipelineConfig


def test_pipeline_config():
    """PipelineConfig holds name and task list."""
    p = PipelineConfig(name="ship", tasks=["implement", "review"])

    assert p.name == "ship"
    assert p.tasks == ["implement", "review"]


def test_pipeline_config_with_push_pr():
    """PipelineConfig can override push/pr settings."""
    p = PipelineConfig(name="ship", tasks=["implement"], push=True, pr=True)

    assert p.push is True
    assert p.pr is True


def test_pipeline_config_defaults_none():
    """PipelineConfig push/pr default to None."""
    p = PipelineConfig(name="ship", tasks=["implement"])

    assert p.push is None
    assert p.pr is None
