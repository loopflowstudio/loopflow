import subprocess
from pathlib import Path

import pytest
import yaml

from scripts import check_actions_secrets, publish_release
from scripts.check_actions_secrets import Environment, SecretRead, Settings

ROOT = Path(__file__).resolve().parents[2]
SIGNING = frozenset({"NOTARY_KEY", "NOTARY_KEY_ID"})
READS = [
    SecretRead("main-packages.yml", "desktop-app", "signing", SIGNING),
    SecretRead("notarization-check.yml", "notarytool", "signing", SIGNING),
]


def _settings(
    *,
    repository: set[str] = frozenset(),
    organization: set[str] = frozenset(),
    signing: Environment | None = Environment(frozenset({"main"}), SIGNING),
    others: dict[str, Environment] = {},
) -> Settings:
    environments = {**others, **({"signing": signing} if signing else {})}
    return Settings(frozenset(repository), frozenset(organization), environments)


def _write(tmp_path: Path, name: str, workflow: dict) -> None:
    (tmp_path / name).write_text(yaml.safe_dump(workflow))


def test_every_secret_a_workflow_reads_comes_from_an_environment():
    reads = check_actions_secrets.secret_reads()

    assert reads
    assert [read for read in reads if read.environment is None] == []


def test_packages_sign_only_a_push_to_main_that_passed_ci():
    text = (ROOT / ".github/workflows/main-packages.yml").read_text()
    workflow = yaml.load(text, Loader=yaml.BaseLoader)
    head = "${{ github.event.workflow_run.head_sha }}"

    assert set(workflow["on"]) == {"workflow_run"}
    assert workflow["on"]["workflow_run"]["branches"] == ["main"]
    assert "github.sha" not in text

    packages = workflow["jobs"]["packages"]
    assert "github.event.workflow_run.conclusion == 'success' &&" in packages["if"]
    assert "&& github.event.workflow_run.event == 'push'" in packages["if"]
    assert "||" not in packages["if"]
    assert packages["with"]["revision"] == head

    signing = [job for job in workflow["jobs"].values() if "environment" in job]
    assert signing and all(job["needs"] == "packages" for job in signing)
    checkouts = [
        step
        for job in signing
        for step in job["steps"]
        if step.get("uses", "").startswith("actions/checkout@")
    ]
    assert checkouts and all(step["with"]["ref"] == head for step in checkouts)


@pytest.mark.parametrize(
    "job",
    [
        {"env": {"KEY": "${{ secrets.NOTARY_KEY }}"}},
        {"uses": "./.github/workflows/package-build.yml", "secrets": "inherit"},
        {"steps": [{"run": "echo '${{ toJSON(secrets) }}'"}]},
        {"steps": [{"run": "echo ${{ secrets['NOTARY_KEY'] }}"}]},
    ],
)
def test_a_job_reading_secrets_without_an_environment_is_reported(tmp_path, job):
    _write(tmp_path, "leak.yml", {"on": "push", "jobs": {"build": job}})

    [read] = check_actions_secrets.secret_reads(tmp_path)

    assert (read.job, read.environment) == ("build", None)
    [line] = check_actions_secrets.differences([read], _settings())
    assert line.startswith("leak.yml job build: reads ")
    assert line.endswith(" without naming an environment")


def test_a_workflow_level_secret_is_reported(tmp_path):
    job = {"environment": "signing", "steps": [{"run": "true"}]}
    _write(
        tmp_path,
        "leak.yml",
        {"on": "push", "env": {"KEY": "${{ secrets.NOTARY_KEY }}"}, "jobs": {"build": job}},
    )

    [read] = check_actions_secrets.secret_reads(tmp_path)

    assert check_actions_secrets.differences([read], _settings()) == [
        "leak.yml outside its jobs: reads NOTARY_KEY without naming an environment"
    ]


def test_the_job_token_needs_no_environment(tmp_path):
    job = {"steps": [{"run": "gh pr list", "env": {"GH_TOKEN": "${{ secrets.GITHUB_TOKEN }}"}}]}
    _write(tmp_path, "token.yml", {"on": "push", "jobs": {"build": job}})

    assert check_actions_secrets.secret_reads(tmp_path) == []


def test_matching_settings_print_nothing():
    assert check_actions_secrets.differences(READS, _settings()) == []


def test_an_open_environment_holding_nothing_is_not_reported():
    settings = _settings(others={"pypi": Environment(None, frozenset())})

    assert check_actions_secrets.differences(READS, settings) == []


@pytest.mark.parametrize(
    ("settings", "line"),
    [
        (
            _settings(repository={"PROBE"}),
            "repository secret PROBE: any branch can read it; "
            "move it to an environment or delete it",
        ),
        (
            _settings(organization={"SHARED"}),
            "organization secret SHARED: any branch can read it; "
            "move it to an environment or stop sharing it with this repository",
        ),
        (
            _settings(signing=None),
            "environment signing: does not exist; the first job naming it creates it open",
        ),
        (
            _settings(signing=Environment(None, SIGNING)),
            "environment signing: no branch policy by name; expected main only",
        ),
        (
            _settings(signing=Environment(frozenset({"main", "release/*"}), SIGNING)),
            "environment signing: branch policy allows release/*; expected main only",
        ),
        (
            _settings(signing=Environment(frozenset({"tag main"}), SIGNING)),
            "environment signing: branch policy allows tag main; expected main only",
        ),
        (
            _settings(signing=Environment(frozenset({"main"}), frozenset({"NOTARY_KEY"}))),
            "environment signing: NOTARY_KEY_ID is named by main-packages.yml, "
            "notarization-check.yml and is not set",
        ),
        (
            _settings(others={"dev": Environment(None, frozenset({"WEBHOOK_SECRET"}))}),
            "environment dev: no branch policy by name; expected main only",
        ),
    ],
)
def test_each_changed_setting_is_named(settings, line):
    assert check_actions_secrets.differences(READS, settings) == [line]


def _fake_gh(responses: dict[str, tuple[int, str, str]]):
    return lambda path, jq: responses.get(path, (1, "", "gh: Not Found (HTTP 404)"))


def test_live_settings_are_read_by_name(monkeypatch):
    monkeypatch.setattr(
        check_actions_secrets,
        "_gh",
        _fake_gh(
            {
                "actions/secrets": (0, "PROBE\n", ""),
                "actions/organization-secrets": (0, "", ""),
                "environments": (0, "false open\ntrue signing\nfalse two words\n", ""),
                "environments/signing/deployment-branch-policies": (0, "main\ntag v*\n", ""),
                "environments/signing/secrets": (0, "NOTARY_KEY\n", ""),
                "environments/open/secrets": (0, "", ""),
                "environments/two%20words/secrets": (0, "WEBHOOK_SECRET\n", ""),
            }
        ),
    )

    assert check_actions_secrets.read_settings() == Settings(
        frozenset({"PROBE"}),
        frozenset(),
        {
            "open": Environment(None, frozenset()),
            "signing": Environment(frozenset({"main", "tag v*"}), frozenset({"NOTARY_KEY"})),
            "two words": Environment(None, frozenset({"WEBHOOK_SECRET"})),
        },
    )


def test_a_failed_read_is_not_an_empty_answer(monkeypatch, capsys):
    monkeypatch.setattr(
        check_actions_secrets,
        "_gh",
        _fake_gh({"actions/secrets": (1, "", "gh: Resource not accessible (HTTP 403)")}),
    )

    assert check_actions_secrets.main() == 1
    assert capsys.readouterr().out == (
        "could not read actions/secrets: gh: Resource not accessible (HTTP 403)\n"
    )


def test_a_missing_gh_is_a_failed_read(monkeypatch, capsys):
    def run(*_, **__):
        raise FileNotFoundError("gh")

    monkeypatch.setattr(check_actions_secrets.subprocess, "run", run)

    assert check_actions_secrets.main() == 1
    assert capsys.readouterr().out == "could not read actions/secrets: gh is not installed\n"


def test_the_release_preflight_shows_why_the_check_crashed(monkeypatch):
    def run(cmd, **_):
        if cmd[-1].endswith("check_actions_secrets.py"):
            return subprocess.CompletedProcess(cmd, 1, stdout="", stderr="No module named 'yaml'\n")
        return subprocess.CompletedProcess(cmd, 0, stdout="Developer ID Application", stderr="")

    monkeypatch.setattr(publish_release, "_run", run)
    monkeypatch.setattr(publish_release.shutil, "which", lambda command: command)
    monkeypatch.setattr(publish_release.platform, "system", lambda: "Darwin")
    monkeypatch.setattr(publish_release.platform, "machine", lambda: "arm64")
    for name in publish_release.REQUIRED_SECRETS:
        monkeypatch.setenv(name, "set")

    with pytest.raises(RuntimeError, match="No module named 'yaml'"):
        publish_release.check_release_host()


def test_the_release_preflight_stops_on_a_changed_setting(monkeypatch):
    line = "environment signing: branch policy allows release/*; expected main only"

    def run(cmd, **_):
        if cmd[-1].endswith("check_actions_secrets.py"):
            return subprocess.CompletedProcess(cmd, 1, stdout=line + "\n", stderr="")
        return subprocess.CompletedProcess(cmd, 0, stdout="Developer ID Application", stderr="")

    monkeypatch.setattr(publish_release, "_run", run)
    monkeypatch.setattr(publish_release.shutil, "which", lambda command: command)
    monkeypatch.setattr(publish_release.platform, "system", lambda: "Darwin")
    monkeypatch.setattr(publish_release.platform, "machine", lambda: "arm64")
    for name in publish_release.REQUIRED_SECRETS:
        monkeypatch.setenv(name, "set")

    with pytest.raises(RuntimeError) as error:
        publish_release.check_release_host()

    assert str(error.value) == (
        'GitHub Actions secrets are not main-only (.github/CI.md, "Signing secrets"):\n' + line
    )
