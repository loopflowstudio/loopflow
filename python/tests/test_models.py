"""Tests for loopflow wire models."""

from __future__ import annotations

from conftest import (
    AUTH_FLOW,
    AUTH_PROVIDER_ACTIVE,
    AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS,
    AUTH_PROVIDER_APIKEY,
    PROVIDER_INFO_FULL,
    PROVIDER_INFO_MINIMAL,
    REPO_MINIMAL,
    WAVE_FULL,
    WAVE_MINIMAL,
)
from loopflow.models import (
    AuthFlow,
    AuthProviderStatus,
    CostRates,
    ModelInfo,
    ProviderInfo,
    Repo,
    Wave,
)


class TestWaveModel:
    def test_minimal_payload(self):
        wave = Wave.model_validate(WAVE_MINIMAL)
        assert wave.name == "reduce"
        assert wave.created_at is None
        assert wave.flow_steps == []
        assert wave.task_capacity == 1
        assert wave.repo == "/tmp/repo"
        assert wave.iteration == 0
        assert wave.agent is None
        assert wave.skill_agents is None

    def test_full_payload(self):
        wave = Wave.model_validate(WAVE_FULL)
        assert wave.created_at is not None
        assert wave.agent == "codex"
        assert wave.skill_agents == {"gate": "claude"}
        assert [skill.type for skill in wave.flow_steps] == ["skill", "skill", "skill", "skill"]
        assert [skill.name for skill in wave.flow_steps] == [
            "review",
            "iterate",
            "build",
            "gate",
        ]

    def test_round_trip(self):
        wave = Wave.model_validate(WAVE_FULL)
        dumped = wave.model_dump(mode="json")
        reparsed = Wave.model_validate(dumped)
        assert reparsed.id == wave.id
        assert reparsed.flow_steps == wave.flow_steps

    def test_unknown_fields_ignored(self):
        data = {**WAVE_MINIMAL, "new_field": "surprise"}
        wave = Wave.model_validate(data)
        assert wave.name == "reduce"

    def test_flow_steps_parse_ops_items(self):
        data = {**WAVE_MINIMAL, "flow_steps": ["implement", "op: pr land --create-pr"]}
        wave = Wave.model_validate(data)
        assert [skill.type for skill in wave.flow_steps] == ["skill", "op"]
        assert wave.flow_steps[1].name == "pr land --create-pr"

class TestAuthModels:
    def test_auth_provider_status(self):
        status = AuthProviderStatus.model_validate(AUTH_PROVIDER_ACTIVE)
        assert status.provider == "github"
        assert status.status == "active"
        assert status.login == "jackdanger"
        assert status.expires_at is None
        assert status.next_refresh_at is None
        assert status.credential_type == "oauth"

    def test_auth_provider_status_with_timestamps(self):
        status = AuthProviderStatus.model_validate(AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS)
        assert status.expires_at is not None
        assert status.next_refresh_at is not None
        assert status.credential_type == "oauth"

    def test_auth_provider_apikey(self):
        status = AuthProviderStatus.model_validate(AUTH_PROVIDER_APIKEY)
        assert status.provider == "codex"
        assert status.status == "active"
        assert status.credential_type == "apikey"
        assert status.login is None

    def test_auth_flow(self):
        flow = AuthFlow.model_validate(AUTH_FLOW)
        assert flow.provider == "github"
        assert flow.verification_uri.startswith("https://")
        assert flow.user_code == "ABCD-1234"


class TestRepoModel:
    def test_minimal_payload(self):
        repo = Repo.model_validate(REPO_MINIMAL)
        assert repo.path == "/tmp/repo"
        assert repo.repo_id == "loopflowstudio/repo"
        assert repo.registered is True
        assert repo.added_at is not None


class TestProviderModel:
    def test_minimal_payload(self):
        info = ProviderInfo.model_validate(PROVIDER_INFO_MINIMAL)
        assert info.provider == "codex"
        assert info.auth_status == "none"
        assert info.billing == "subscription"
        assert len(info.models) == 1
        assert info.models[0].id == "gpt-5.1-codex"
        assert info.models[0].cost_rates is None

    def test_full_payload(self):
        info = ProviderInfo.model_validate(PROVIDER_INFO_FULL)
        assert info.provider == "opencodezen"
        assert info.auth_status == "active"
        assert info.login == "user@example.com"
        assert info.models[0].provider == "opencodezen"
        assert info.models[0].cost_rates is not None
        assert info.models[0].cost_rates.output_per_mtok == 1.0

    def test_nested_models_and_cost_rates(self):
        model = ModelInfo.model_validate(PROVIDER_INFO_FULL["models"][0])
        rates = CostRates.model_validate(model.cost_rates.model_dump())
        assert model.display_name == "Kimi K2.5"
        assert rates.input_per_mtok == 0.5
