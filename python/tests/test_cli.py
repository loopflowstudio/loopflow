"""Tests for loopflow.cli table rendering."""

from __future__ import annotations

import pytest
import typer
from conftest import (
    AUTH_PROVIDER_ACTIVE,
    AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS,
    AUTH_PROVIDER_NONE,
    PROVIDER_INFO_FULL,
    PROVIDER_INFO_MINIMAL,
    REPO_MINIMAL,
    USAGE_SUMMARY,
    USAGE_SUMMARY_BY_MODEL,
    WAVE_FULL,
    WAVE_MINIMAL,
)
from loopflow.cli import (
    _auth_poll_timeout_seconds,
    _auth_status_table,
    _billing_tables,
    _estimate_cost,
    _extract_authorization_code,
    _format_cost,
    _format_tokens,
    _infer_group_by,
    _providers_table,
    _repo_table,
    _split_repo_slug,
    _status_details,
    _usage_table,
    _wave_detail_table,
    _wave_table,
)
from loopflow.models import (
    AuthProviderStatus,
    CostRates,
    ProviderInfo,
    Repo,
    TokenTotals,
    UsageSummary,
    Wave,
)
from rich.console import Console


def _render_table(wave: Wave) -> str:
    console = Console(record=True, width=220)
    console.print(_wave_table([wave]))
    return console.export_text()


def test_wave_table_includes_worktree_and_branch_columns() -> None:
    wave = Wave.model_validate(WAVE_MINIMAL)
    rendered = _render_table(wave)

    assert "local_worktree" in rendered
    assert "remote_branch" in rendered


def test_wave_table_uses_active_run_paths_when_available() -> None:
    wave = Wave.model_validate(WAVE_FULL)
    rendered = _render_table(wave)

    assert "/tmp/wt" in rendered
    assert "wave/reduce" in rendered


def test_wave_table_falls_back_to_wave_branch() -> None:
    payload = dict(WAVE_MINIMAL)
    payload["remote_branch"] = "wave/fallback"
    payload["local_worktree"] = "/tmp/fallback-wt"
    wave = Wave.model_validate(payload)
    rendered = _render_table(wave)

    assert "wave/fallback" in rendered
    assert "/tmp/fallback-wt" in rendered


def test_wave_detail_table_includes_flow_and_area() -> None:
    wave = Wave.model_validate(
        {
            **WAVE_MINIMAL,
            "area": ["wave/chord-model/", "wave/signals/"],
            "direction": ["care", "clarity"],
            "primary_flow": "tend",
        }
    )
    console = Console(record=True, width=220)
    console.print(_wave_detail_table(wave))
    rendered = console.export_text()

    assert "flow" in rendered
    assert "tend" in rendered
    assert "area" in rendered
    assert "wave/chord-model/, wave/signals/" in rendered


def test_auth_status_table_shows_active_and_none_states() -> None:
    statuses = [
        AuthProviderStatus.model_validate(AUTH_PROVIDER_ACTIVE),
        AuthProviderStatus.model_validate(AUTH_PROVIDER_NONE),
    ]
    console = Console(record=True, width=220)
    console.print(_auth_status_table(statuses))
    rendered = console.export_text()

    assert "GitHub" in rendered
    assert "@jackdanger" in rendered
    assert "OpenCode Zen" in rendered
    assert "not connected" in rendered


def test_auth_status_table_shows_expiry_details() -> None:
    statuses = [AuthProviderStatus.model_validate(AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS)]
    console = Console(record=True, width=220)
    console.print(_auth_status_table(statuses))
    rendered = console.export_text()

    assert "Claude" in rendered
    assert "jack@anthropic.com" in rendered
    assert "expires" in rendered
    assert "refresh in" in rendered


def test_auth_status_table_shows_new_pm_providers() -> None:
    statuses = [
        AuthProviderStatus.model_validate({"provider": "asana", "status": "none"}),
        AuthProviderStatus.model_validate({"provider": "linear", "status": "active"}),
        AuthProviderStatus.model_validate({"provider": "notion", "status": "active"}),
    ]
    console = Console(record=True, width=220)
    console.print(_auth_status_table(statuses))
    rendered = console.export_text()

    assert "Asana" in rendered
    assert "Linear" in rendered
    assert "Notion" in rendered


def test_auth_status_table_does_not_mark_pm_api_keys_as_metered() -> None:
    statuses = [
        AuthProviderStatus.model_validate(
            {"provider": "asana", "status": "active", "credential_type": "apikey"}
        )
    ]
    console = Console(record=True, width=220)
    console.print(_auth_status_table(statuses))
    rendered = console.export_text()

    assert "Asana" in rendered
    assert "pay-per-token" not in rendered


def test_auth_poll_timeout_uses_provider_expiry_when_present() -> None:
    assert _auth_poll_timeout_seconds(900) == 900


def test_auth_poll_timeout_falls_back_for_missing_or_invalid_expiry() -> None:
    assert _auth_poll_timeout_seconds(None) == 180
    assert _auth_poll_timeout_seconds(0) == 180


def test_extract_authorization_code_accepts_raw_code() -> None:
    assert _extract_authorization_code("abc123") == "abc123"


def test_extract_authorization_code_parses_redirect_url() -> None:
    assert (
        _extract_authorization_code("urn:ietf:wg:oauth:2.0:oob?code=abc123&state=ignored")
        == "abc123"
    )


def test_repo_table_shows_registration_columns() -> None:
    repo = Repo.model_validate(REPO_MINIMAL)
    console = Console(record=True, width=220)
    console.print(_repo_table([repo]))
    rendered = console.export_text()

    assert "repo_id" in rendered
    assert "registered" in rendered
    assert "added_at" in rendered
    assert "yes" in rendered


def test_auth_status_shows_refreshing_soon_when_past_refresh_time() -> None:
    status = AuthProviderStatus.model_validate(
        {
            "provider": "claude",
            "status": "active",
            "login": "jack@anthropic.com",
            "expires_at": "2030-01-01T04:00:00Z",
            "next_refresh_at": "2020-01-01T00:00:00Z",
        }
    )
    details = _status_details(status)
    assert "refreshing soon" in details


def test_auth_status_no_refresh_when_no_next_refresh_at() -> None:
    status = AuthProviderStatus.model_validate(AUTH_PROVIDER_ACTIVE)
    details = _status_details(status)
    assert "refresh" not in details


def test_split_repo_slug_parses_owner_repo() -> None:
    assert _split_repo_slug("loopflowstudio/loopflow") == ("loopflowstudio", "loopflow")


# Token formatting


def test_format_tokens_zero_shows_dash() -> None:
    assert _format_tokens(0) == "\u2014"


def test_format_tokens_small_shows_raw() -> None:
    assert _format_tokens(42) == "42"
    assert _format_tokens(999) == "999"


def test_format_tokens_thousands_shows_k() -> None:
    assert _format_tokens(1000) == "1.0k"
    assert _format_tokens(42100) == "42.1k"
    assert _format_tokens(128300) == "128.3k"


def test_format_tokens_millions_shows_m() -> None:
    assert _format_tokens(1_000_000) == "1.0M"
    assert _format_tokens(1_200_000) == "1.2M"


# Usage table


def test_usage_table_renders_groups() -> None:
    summary = UsageSummary.model_validate(USAGE_SUMMARY)
    console = Console(record=True, width=220)
    console.print(_usage_table(summary))
    rendered = console.export_text()

    assert "engbot" in rendered
    assert "infra" in rendered
    assert "42.1k" in rendered
    assert "128.3k" in rendered
    assert "12.0k" in rendered
    assert "45.0k" in rendered
    assert "\u2014" in rendered


def test_usage_table_column_header_matches_group_by() -> None:
    payload = dict(USAGE_SUMMARY)
    payload["group_by"] = "step"
    summary = UsageSummary.model_validate(payload)
    console = Console(record=True, width=220)
    console.print(_usage_table(summary))
    rendered = console.export_text()

    assert "step" in rendered


# Group-by inference


def test_infer_group_by_wave_filter_gives_step() -> None:
    result = _infer_group_by(
        wave="engbot",
        flow=None,
        step=None,
        model=None,
        source=None,
        prompt=False,
        group_by=None,
    )
    assert result == "step"


def test_infer_group_by_flow_filter_gives_wave() -> None:
    result = _infer_group_by(
        wave=None,
        flow="build",
        step=None,
        model=None,
        source=None,
        prompt=False,
        group_by=None,
    )
    assert result == "wave"


def test_infer_group_by_source_filter_gives_wave() -> None:
    result = _infer_group_by(
        wave=None,
        flow=None,
        step=None,
        model=None,
        source="scratch",
        prompt=False,
        group_by=None,
    )
    assert result == "wave"


def test_infer_group_by_prompt_overrides() -> None:
    result = _infer_group_by(
        wave=None,
        flow=None,
        step=None,
        model=None,
        source=None,
        prompt=True,
        group_by=None,
    )
    assert result == "source"


def test_infer_group_by_explicit_wins() -> None:
    result = _infer_group_by(
        wave="x",
        flow=None,
        step=None,
        model=None,
        source=None,
        prompt=False,
        group_by="model",
    )
    assert result == "model"


def test_infer_group_by_multiple_filters_requires_explicit() -> None:
    with pytest.raises(typer.Exit):
        _infer_group_by(
            wave="x",
            flow="y",
            step=None,
            model=None,
            source=None,
            prompt=False,
            group_by=None,
        )


def test_infer_group_by_no_filter_gives_wave() -> None:
    result = _infer_group_by(
        wave=None,
        flow=None,
        step=None,
        model=None,
        source=None,
        prompt=False,
        group_by=None,
    )
    assert result == "wave"


# Providers table


def test_providers_table_renders_providers() -> None:
    providers = [
        ProviderInfo.model_validate(PROVIDER_INFO_MINIMAL),
        ProviderInfo.model_validate(PROVIDER_INFO_FULL),
    ]
    console = Console(record=True, width=220)
    console.print(_providers_table(providers))
    rendered = console.export_text()

    assert "Codex" in rendered
    assert "OpenCode Zen" in rendered
    assert "subscription" in rendered
    assert "per_token" in rendered
    assert "GPT-5.1 Codex" in rendered
    assert "Kimi K2.5" in rendered
    assert "\u2713 active" in rendered
    assert "\u2717 none" in rendered


# Cost estimation


def test_estimate_cost_input_and_output() -> None:
    tokens = TokenTotals(input=1_000_000, output=500_000)
    rates = CostRates(input_per_mtok=3.0, output_per_mtok=15.0)
    assert _estimate_cost(tokens, rates) == pytest.approx(10.5)


def test_estimate_cost_includes_cache() -> None:
    tokens = TokenTotals(cache_read=1_000_000, cache_write=1_000_000)
    rates = CostRates(
        input_per_mtok=3.0,
        output_per_mtok=15.0,
        cache_read_per_mtok=0.3,
        cache_write_per_mtok=3.75,
    )
    assert _estimate_cost(tokens, rates) == pytest.approx(4.05)


def test_format_cost_below_penny() -> None:
    assert _format_cost(0.001) == "<$0.01"


def test_format_cost_normal() -> None:
    assert _format_cost(4.20) == "~$4.20"


# Billing tables


def test_billing_tables_splits_by_billing_type() -> None:
    summary = UsageSummary.model_validate(USAGE_SUMMARY_BY_MODEL)
    providers = [
        ProviderInfo.model_validate(PROVIDER_INFO_MINIMAL),
        ProviderInfo.model_validate(PROVIDER_INFO_FULL),
    ]
    tables, total_line = _billing_tables(summary, providers)
    assert len(tables) == 2

    console = Console(record=True, width=220)
    for t in tables:
        console.print(t)
    rendered = console.export_text()

    assert "Subscription" in rendered
    assert "Metered" in rendered
    assert "gpt-5.1-codex" in rendered
    assert "opencode/kimi-k2.5" in rendered
    assert "est. cost" in rendered
    assert total_line is not None
    assert "~$1.00" in total_line


def test_billing_tables_subscription_only() -> None:
    summary = UsageSummary.model_validate(USAGE_SUMMARY_BY_MODEL)
    # Only subscription provider — metered model won't match
    providers = [ProviderInfo.model_validate(PROVIDER_INFO_MINIMAL)]
    tables, total_line = _billing_tables(summary, providers)

    # Both groups fall into subscription (unmatched defaults to subscription)
    assert len(tables) == 1
    assert total_line is None

    console = Console(record=True, width=220)
    console.print(tables[0])
    rendered = console.export_text()
    assert "Subscription" in rendered


def test_billing_tables_empty_groups() -> None:
    summary = UsageSummary(group_by="model", groups=[])
    providers = [ProviderInfo.model_validate(PROVIDER_INFO_MINIMAL)]
    tables, total_line = _billing_tables(summary, providers)
    assert tables == []
    assert total_line is None
