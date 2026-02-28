from __future__ import annotations

import pytest
from loopflow.client import Client

from scripts.lib.api_harness import ApiClient
from scripts.lib.lfd_runtime import LfdRuntime
from scripts.lib.wave_scenarios import (
    create_wave_duplicate_name_error,
    create_wave_happy,
    delete_wave_happy,
    delete_wave_missing_error,
    get_wave_happy,
    get_wave_missing_error,
    list_waves_auth_error,
    list_waves_happy,
    update_wave_agent_overrides_happy,
    update_wave_happy,
    update_wave_invalid_status_error,
)

pytestmark = pytest.mark.e2e

_state: dict[str, str] = {}


def test_create_wave_happy(lf_client: Client, lfd_runtime: LfdRuntime) -> None:
    create_wave_happy(lf_client, lfd_runtime, _state)


def test_create_wave_duplicate_name_error(api_client: ApiClient, lfd_runtime: LfdRuntime) -> None:
    create_wave_duplicate_name_error(api_client, lfd_runtime)


def test_list_waves_happy(lf_client: Client) -> None:
    list_waves_happy(lf_client, _state)


def test_list_waves_auth_error(api_client: ApiClient) -> None:
    list_waves_auth_error(api_client)


def test_get_wave_happy(lf_client: Client) -> None:
    get_wave_happy(lf_client, _state)


def test_get_wave_missing_error(api_client: ApiClient) -> None:
    get_wave_missing_error(api_client)


def test_update_wave_happy(lf_client: Client) -> None:
    update_wave_happy(lf_client, _state)


def test_update_wave_invalid_status_error(api_client: ApiClient) -> None:
    update_wave_invalid_status_error(api_client, _state)


def test_update_wave_agent_overrides_happy(api_client: ApiClient) -> None:
    update_wave_agent_overrides_happy(api_client, _state)


def test_delete_wave_happy(lf_client: Client, lfd_runtime: LfdRuntime) -> None:
    delete_wave_happy(lf_client, lfd_runtime, _state)


def test_delete_wave_missing_error(api_client: ApiClient) -> None:
    delete_wave_missing_error(api_client, _state)
