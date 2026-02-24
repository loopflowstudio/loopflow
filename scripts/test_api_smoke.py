#!/usr/bin/env python3
"""Live API smoke suite for wave CRUD endpoints."""

from __future__ import annotations

from functools import partial

from lib.api_harness import ApiClient, ScenarioRunner
from lib.lfd_runtime import LfdRuntime
from lib.wave_scenarios import (
    create_wave_duplicate_name_error,
    create_wave_happy,
    delete_wave_happy,
    delete_wave_missing_error,
    get_wave_happy,
    get_wave_missing_error,
    list_waves_auth_error,
    list_waves_happy,
    update_wave_happy,
    update_wave_invalid_status_error,
)
from loopflow.client import Client


def main() -> int:
    with LfdRuntime() as runtime:
        client = Client(base_url=runtime.base_url, token=runtime.token)
        try:
            with ApiClient(base_url=runtime.base_url, token=runtime.token) as raw:
                runner = ScenarioRunner()
                state: dict[str, str] = {}

                scenarios = [
                    ("create_wave_happy", partial(create_wave_happy, client, runtime, state)),
                    (
                        "create_wave_duplicate_name_error",
                        partial(create_wave_duplicate_name_error, raw, runtime),
                    ),
                    ("list_waves_happy", partial(list_waves_happy, client, state)),
                    ("list_waves_auth_error", partial(list_waves_auth_error, raw)),
                    ("get_wave_happy", partial(get_wave_happy, client, state)),
                    ("get_wave_missing_error", partial(get_wave_missing_error, raw)),
                    ("update_wave_happy", partial(update_wave_happy, client, state)),
                    (
                        "update_wave_invalid_status_error",
                        partial(update_wave_invalid_status_error, raw, state),
                    ),
                    ("delete_wave_happy", partial(delete_wave_happy, client, runtime, state)),
                    ("delete_wave_missing_error", partial(delete_wave_missing_error, raw, state)),
                ]

                for name, check in scenarios:
                    runner.run_scenario(name, check)
                runner.print_summary()

            if runner.has_failures():
                print("lfd logs:\n" + runtime.logs())
                return 1
        finally:
            client.close()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
