"""E2E smoke tests for trigger CRUD endpoints."""

from __future__ import annotations

import uuid

import pytest
from loopflow.client import Client

from scripts.lib.api_harness import ApiClient
from scripts.lib.lfd_runtime import LfdRuntime
from scripts.lib.trigger_scenarios import (
    add_trigger_ci_failure_happy,
    add_trigger_repo_happy,
    list_triggers_empty,
    list_triggers_happy,
    remove_trigger_happy,
)

pytestmark = pytest.mark.e2e

_state: dict[str, str] = {}


def test_setup_wave(lf_client: Client, lfd_runtime: LfdRuntime) -> None:
    name = f"trigger-smoke-{uuid.uuid4().hex[:8]}"
    wave = lf_client.create_wave(name=name, repo=str(lfd_runtime.repo_dir))
    _state["trigger_wave_id"] = wave.id


def test_add_trigger_repo(lf_client: Client) -> None:
    add_trigger_repo_happy(lf_client, _state)


def test_list_triggers_has_one(api_client: ApiClient) -> None:
    list_triggers_happy(api_client, _state)


def test_remove_trigger(lf_client: Client) -> None:
    remove_trigger_happy(lf_client, _state)


def test_list_triggers_empty(api_client: ApiClient) -> None:
    list_triggers_empty(api_client, _state)


def test_add_trigger_ci_failure(lf_client: Client) -> None:
    add_trigger_ci_failure_happy(lf_client, _state)


def test_cleanup(lf_client: Client) -> None:
    lf_client.delete_wave(_state["trigger_wave_id"])
