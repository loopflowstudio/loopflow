#!/usr/bin/env python3
"""Compare GitHub's live Actions secret settings with what the workflows name."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ROOT / ".github/workflows"

_SECRET_NAME = re.compile(r"\bsecrets\.([A-Za-z_][A-Za-z0-9_]*)")
# `secrets: inherit`, `toJSON(secrets)` and `secrets[...]` read secrets too.
_SECRET_USE = re.compile(r"\bsecrets\b(?!\.GITHUB_TOKEN\b)")


# What the workflows name


@dataclass(frozen=True)
class SecretRead:
    workflow: str
    job: str | None  # None: outside any job, where no environment can apply
    environment: str | None
    names: frozenset[str]


def secret_reads(workflows: Path = WORKFLOWS) -> list[SecretRead]:
    reads = []
    for path in sorted([*workflows.glob("*.yml"), *workflows.glob("*.yaml")]):
        workflow = yaml.load(path.read_text(), Loader=yaml.BaseLoader)
        jobs = workflow.pop("jobs", {})
        for job, body in [(None, workflow), *jobs.items()]:
            text = json.dumps(body)
            if not _SECRET_USE.search(text):
                continue
            environment = body.get("environment") if job else None
            if isinstance(environment, dict):
                environment = environment.get("name")
            names = frozenset(_SECRET_NAME.findall(text)) - {"GITHUB_TOKEN"}
            reads.append(SecretRead(path.name, job, environment, names))
    return reads


# What GitHub holds


@dataclass(frozen=True)
class Environment:
    # None: no custom branch policy, so GitHub does not restrict by name.
    allowed_refs: frozenset[str] | None
    secrets: frozenset[str]


@dataclass(frozen=True)
class Settings:
    repository_secrets: frozenset[str]
    organization_secrets: frozenset[str]
    environments: dict[str, Environment | None]  # None: does not exist


class SettingsUnreadable(RuntimeError):
    pass


def _gh(path: str, jq: str) -> tuple[int, str, str]:
    result = subprocess.run(
        ["gh", "api", "--paginate", f"repos/{{owner}}/{{repo}}/{path}", "--jq", jq],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return result.returncode, result.stdout, result.stderr


def _read(path: str, jq: str, *, absent_ok: bool = False) -> list[str] | None:
    status, stdout, stderr = _gh(path, jq)
    if status == 0:
        return stdout.splitlines()
    if absent_ok and "HTTP 404" in stderr:
        return None
    reason = stderr.strip().splitlines()[-1] if stderr.strip() else f"gh exited {status}"
    raise SettingsUnreadable(f"could not read {path}: {reason}")


def _secret_names(path: str) -> frozenset[str]:
    return frozenset(_read(path, ".secrets[].name"))


def _read_environment(name: str) -> Environment | None:
    by_name = _read(
        f"environments/{name}",
        ".deployment_branch_policy.custom_branch_policies == true",
        absent_ok=True,
    )
    if by_name is None:
        return None
    allowed_refs = None
    if by_name == ["true"]:
        rules = _read(
            f"environments/{name}/deployment-branch-policies",
            '.branch_policies[] | if .type == "branch" then .name else "\\(.type) \\(.name)" end',
        )
        allowed_refs = frozenset(rules)
    return Environment(allowed_refs, _secret_names(f"environments/{name}/secrets"))


def read_settings(environments: set[str]) -> Settings:
    return Settings(
        repository_secrets=_secret_names("actions/secrets"),
        organization_secrets=_secret_names("actions/organization-secrets"),
        environments={name: _read_environment(name) for name in sorted(environments)},
    )


# Where they disagree


def differences(reads: list[SecretRead], settings: Settings) -> list[str]:
    lines = []
    for name in sorted(settings.repository_secrets):
        lines.append(
            f"repository secret {name}: any branch can read it; "
            "move it to an environment or delete it"
        )
    for name in sorted(settings.organization_secrets):
        lines.append(
            f"organization secret {name}: any branch can read it; "
            "move it to an environment or stop sharing it with this repository"
        )

    named: dict[str, dict[str, set[str]]] = {}
    for read in reads:
        if read.environment is None:
            where = f"job {read.job}" if read.job else "outside its jobs"
            what = ", ".join(sorted(read.names)) or "secrets"
            lines.append(f"{read.workflow} {where}: reads {what} without naming an environment")
            continue
        workflows = named.setdefault(read.environment, {})
        for name in read.names:
            workflows.setdefault(name, set()).add(read.workflow)

    for environment, workflows in sorted(named.items()):
        prefix = f"environment {environment}:"
        live = settings.environments[environment]
        if live is None:
            lines.append(f"{prefix} does not exist; the first job naming it creates it open")
            continue
        if live.allowed_refs is None:
            lines.append(f"{prefix} no branch policy by name; expected main only")
        elif live.allowed_refs != {"main"}:
            extra = ", ".join(sorted(live.allowed_refs - {"main"}))
            allows = f"allows {extra}" if extra else "allows no branch"
            lines.append(f"{prefix} branch policy {allows}; expected main only")
        for name in sorted(workflows.keys() - live.secrets):
            lines.append(
                f"{prefix} {name} is named by {', '.join(sorted(workflows[name]))} and is not set"
            )
        for name in sorted(live.secrets - workflows.keys()):
            lines.append(f"{prefix} {name} is set and no job names it")
    return lines


def main() -> int:
    reads = secret_reads()
    try:
        settings = read_settings({read.environment for read in reads if read.environment})
    except SettingsUnreadable as error:
        print(error)
        return 1
    lines = differences(reads, settings)
    print("\n".join(lines), end="\n" if lines else "")
    return 1 if lines else 0


if __name__ == "__main__":
    sys.exit(main())
