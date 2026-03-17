"""Tests for loopflow.models — Wave and WaveRun parsing."""

from __future__ import annotations

from conftest import (
    AUTH_FLOW,
    AUTH_PROVIDER_ACTIVE,
    AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS,
    AUTH_PROVIDER_APIKEY,
    PROVIDER_INFO_FULL,
    PROVIDER_INFO_MINIMAL,
    REPO_MINIMAL,
    SESSION_FULL,
    SESSION_MINIMAL,
    WAVE_FULL,
    WAVE_MINIMAL,
    WAVE_RUN_MINIMAL,
)
from loopflow.models import (
    AuthFlow,
    AuthProviderStatus,
    CommitEntry,
    CostRates,
    ModelInfo,
    ProviderInfo,
    Repo,
    Session,
    SessionEventEnvelope,
    Wave,
    WaveRun,
)


class TestWaveModel:
    def test_minimal_payload(self):
        wave = Wave.model_validate(WAVE_MINIMAL)
        assert wave.name == "reduce"
        assert wave.triggers == []
        assert wave.active_run is None
        assert wave.created_at is None
        assert wave.commits == []
        assert wave.diff_stat is None
        assert wave.flow_steps == []

    def test_full_payload(self):
        wave = Wave.model_validate(WAVE_FULL)
        assert wave.triggers[0].signal == "repo"
        assert wave.active_run.status == "running"
        assert wave.active_run.pr.number == 1
        assert wave.created_at is not None
        assert wave.branch == "wave/reduce"
        assert len(wave.commits) == 2
        assert wave.commits[0].sha == "a1b2c3d"
        assert wave.commits[0].message == "implement: add retry logic"
        assert wave.diff_stat == " 3 files changed, 42 insertions(+), 7 deletions(-)"
        assert [step.type for step in wave.flow_steps] == ["step", "step", "step", "step"]
        assert [step.name for step in wave.flow_steps] == [
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
        assert reparsed.active_run.pr.url == wave.active_run.pr.url
        assert reparsed.commits == wave.commits
        assert reparsed.diff_stat == wave.diff_stat
        assert reparsed.flow_steps == wave.flow_steps

    def test_unknown_fields_ignored(self):
        data = {**WAVE_MINIMAL, "new_field": "surprise"}
        wave = Wave.model_validate(data)
        assert wave.name == "reduce"

    def test_flow_steps_parse_ops_items(self):
        data = {**WAVE_MINIMAL, "flow_steps": ["implement", "ops: land --create-pr"]}
        wave = Wave.model_validate(data)
        assert [step.type for step in wave.flow_steps] == ["step", "ops"]
        assert wave.flow_steps[1].name == "land --create-pr"


class TestCommitEntryModel:
    def test_parse(self):
        entry = CommitEntry.model_validate({"sha": "abc123", "message": "fix bug"})
        assert entry.sha == "abc123"
        assert entry.message == "fix bug"


class TestWaveRunModel:
    def test_minimal_payload(self):
        run = WaveRun.model_validate(WAVE_RUN_MINIMAL)
        assert run.pr is None
        assert run.error is None
        assert run.flow_parents == []

    def test_with_error(self):
        data = {**WAVE_RUN_MINIMAL, "error": "rebase conflict"}
        run = WaveRun.model_validate(data)
        assert run.error == "rebase conflict"


class TestSessionModel:
    def test_minimal_payload(self):
        session = Session.model_validate(SESSION_MINIMAL)
        assert session.harness == "claude"
        assert session.config.yolo_mode is False
        assert session.created_at is None

    def test_full_payload(self):
        session = Session.model_validate(SESSION_FULL)
        assert session.wave_run_id == "run-1"
        assert session.provider_session_id == "provider-1"
        assert session.config.agent == "claude-sonnet-4-5-20250929"
        assert session.created_at is not None
        assert session.ended_at is not None


class TestSessionEventEnvelopeModel:
    def test_parse(self):
        event = SessionEventEnvelope.model_validate(
            {"seq": 12, "event": {"type": "turn_completed", "status": "completed"}}
        )
        assert event.seq == 12
        assert event.event["type"] == "turn_completed"


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
