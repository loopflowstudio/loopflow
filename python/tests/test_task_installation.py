"""Image acquisition must not mistake registry throttling for failed installation proofs."""

import subprocess
from unittest.mock import Mock

import pytest

from scripts import test_task_installation as installation

IMAGE = "public.ecr.aws/docker/library/rust:trixie"
MISSING = subprocess.CompletedProcess([], 1)
PULLED = subprocess.CompletedProcess([], 0, "Downloaded proof image\n")
THROTTLED = subprocess.CompletedProcess([], 1, "toomanyrequests: Rate exceeded\n")


def test_throttled_pull_recovers_before_proofs(monkeypatch, capsys):
    monkeypatch.setattr(
        installation.subprocess, "run", Mock(side_effect=[MISSING, THROTTLED, PULLED])
    )
    monkeypatch.setattr(installation.time, "sleep", lambda _: None)

    installation._prepare_image(IMAGE)

    output = capsys.readouterr().out
    assert f"Registry rate limit for {IMAGE}; retrying in 15 seconds." in output
    assert "Downloaded proof image" in output


def test_cached_image_needs_no_registry(monkeypatch, capsys):
    monkeypatch.setattr(installation.subprocess, "run", Mock(side_effect=[PULLED]))

    installation._prepare_image(IMAGE)

    assert f"Using local proof image {IMAGE}" in capsys.readouterr().out


def test_persistent_throttling_is_bounded_infrastructure_failure(monkeypatch, capsys):
    monkeypatch.setattr(
        installation.subprocess, "run", Mock(side_effect=[MISSING] + [THROTTLED] * 4)
    )
    monkeypatch.setattr(installation.time, "sleep", lambda _: None)

    with pytest.raises(SystemExit, match="Registry rate limit persisted") as error:
        installation._prepare_image(IMAGE)

    assert f"Infrastructure failure acquiring proof image {IMAGE}" in str(error.value)
    assert "installation tests did not run" in str(error.value)
    assert "toomanyrequests: Rate exceeded" in capsys.readouterr().out


def test_non_rate_failure_is_not_retried(monkeypatch, capsys):
    denied = subprocess.CompletedProcess([], 1, "manifest unknown\n")
    monkeypatch.setattr(installation.subprocess, "run", Mock(side_effect=[MISSING, denied]))

    with pytest.raises(SystemExit, match="Infrastructure failure"):
        installation._prepare_image("example.invalid/missing:tag")

    assert "manifest unknown" in capsys.readouterr().out


def test_pull_timeout_reports_infrastructure(monkeypatch):
    monkeypatch.setattr(
        installation.subprocess,
        "run",
        Mock(side_effect=[MISSING, subprocess.TimeoutExpired(["docker", "pull"], 300)]),
    )

    with pytest.raises(SystemExit, match="Docker image acquisition timed out"):
        installation._prepare_image(IMAGE)


def test_backoff_cannot_extend_download_deadline(monkeypatch):
    monkeypatch.setattr(installation.subprocess, "run", Mock(side_effect=[MISSING, THROTTLED]))
    monkeypatch.setattr(installation.time, "monotonic", Mock(side_effect=[0, 0, 290]))

    with pytest.raises(SystemExit, match="Registry rate limit persisted"):
        installation._prepare_image(IMAGE)
