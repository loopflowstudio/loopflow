#!/usr/bin/env python3
"""Verify the configured Asana team is canonical for the wave set.

Usage:
    uv run python scripts/verify_canonical_waves.py

Requires: lfd running, valid Asana auth, repo at the script's CWD.
"""

from __future__ import annotations

import atexit
import json
import os
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Optional

import httpx
import yaml

ASANA_API = "https://app.asana.com/api/1.0"


class StepFailure(RuntimeError):
    def __init__(self, step: int, message: str):
        super().__init__(f"Step {step} failed: {message}")
        self.step = step


@dataclass
class Snapshot:
    path: Path
    content: bytes | None

    @classmethod
    def capture(cls, path: Path) -> "Snapshot":
        return cls(path=path, content=path.read_bytes() if path.exists() else None)

    def restore(self) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        if self.content is None:
            if self.path.exists():
                self.path.unlink()
            return
        self.path.write_bytes(self.content)


@dataclass
class AsanaProject:
    gid: str
    name: str


class AsanaApi:
    def __init__(self, token: str) -> None:
        self.client = httpx.Client(
            base_url=ASANA_API,
            headers={"Authorization": f"Bearer {token}"},
            timeout=30,
        )

    def close(self) -> None:
        self.client.close()

    def _request(self, method: str, path: str, **kwargs: Any) -> Any:
        response = self.client.request(method, path, **kwargs)
        if response.status_code >= 300:
            raise RuntimeError(
                f"Asana {method} {path} failed: {response.status_code} {response.text}"
            )
        if not response.content:
            return None
        payload = response.json()
        return payload.get("data")

    def list_team_projects(self, team_id: str) -> list[AsanaProject]:
        data = self._request("GET", f"/teams/{team_id}/projects", params={"opt_fields": "name"})
        return [AsanaProject(gid=str(item["gid"]), name=str(item["name"])) for item in data or []]

    def create_team(self, workspace_id: str, name: str) -> str:
        data = self._request(
            "POST",
            "/teams",
            json={"data": {"name": name, "organization": workspace_id}},
        )
        return str(data["gid"])

    def delete_team(self, team_id: str) -> None:
        response = self.client.delete(f"/teams/{team_id}")
        if response.status_code in (200, 204, 404):
            return
        raise RuntimeError(
            f"Asana DELETE /teams/{team_id} failed: {response.status_code} {response.text}"
        )



def global_config_path() -> Path:
    lf_home = os.environ.get("LF_HOME")
    if lf_home:
        return Path(lf_home) / "config.yaml"
    return Path.home() / ".lf" / "config.yaml"



def credentials_dir() -> Path:
    lf_home = os.environ.get("LF_HOME")
    if lf_home:
        return Path(lf_home) / "credentials"
    return Path.home() / ".lf" / "credentials"



def load_yaml(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    data = yaml.safe_load(path.read_text())
    return data if isinstance(data, dict) else {}



def write_yaml(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(yaml.safe_dump(data, sort_keys=False))



def read_asana_token() -> str:
    path = credentials_dir() / "asana.json"
    if not path.exists():
        raise StepFailure(0, f"No Asana credentials at {path}. Run `lf op auth asana` first.")
    token = json.loads(path.read_text()).get("access_token")
    if not token:
        raise StepFailure(0, f"No access_token in {path}.")
    return str(token)



def parse_pm_list(stdout: str) -> list[tuple[str, str]]:
    rows: list[tuple[str, str]] = []
    for raw_line in stdout.splitlines():
        line = raw_line.strip()
        if not line or line == "no projects in team":
            continue
        left, _, _right = line.partition(" — ")
        parts = left.split(" ", 1)
        if len(parts) != 2:
            continue
        rows.append((parts[0], parts[1]))
    return rows



def assert_pm_list_matches(step: int, stdout: str, expected: list[AsanaProject]) -> None:
    actual = sorted(parse_pm_list(stdout))
    wanted = sorted((project.gid, project.name) for project in expected)
    if actual != wanted:
        raise StepFailure(
            step,
            f"`lf op pm list` mismatch. expected={wanted!r} actual={actual!r} stdout={stdout!r}",
        )



def run_pm_list(repo: Path) -> str:
    lf_bin = os.environ.get("LF_BIN", "lf")
    result = subprocess.run(
        [lf_bin, "op", "pm", "list"],
        cwd=repo,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip() or "lf op pm list failed")
    return result.stdout



def remove_repo_override(config: dict[str, Any], repo_path: str) -> dict[str, Any]:
    repos = config.get("repos")
    if isinstance(repos, dict):
        repos.pop(repo_path, None)
        if not repos:
            config.pop("repos", None)
    return config



def main() -> int:
    repo = Path.cwd().resolve()
    repo_config_path = repo / ".lf" / "config.yaml"
    global_path = global_config_path()
    repo_snapshot = Snapshot.capture(repo_config_path)
    global_snapshot = Snapshot.capture(global_path)
    snapshots_restored = False
    override_team: Optional[str] = None
    api = AsanaApi(read_asana_token())

    def restore() -> None:
        nonlocal snapshots_restored
        if snapshots_restored:
            return
        repo_snapshot.restore()
        global_snapshot.restore()
        snapshots_restored = True

    atexit.register(restore)

    try:
        # Step 1: snapshot already captured.

        # Step 2.
        repo_config = load_yaml(repo_config_path)
        repo_team = ((repo_config.get("asana") or {}).get("team"))
        if not repo_team:
            raise StepFailure(
                2,
                f"{repo_config_path} is missing asana.team. Set the repo's canonical team first.",
            )

        global_config = load_yaml(global_path)
        effective_workspace = (
            ((repo_config.get("asana") or {}).get("workspace"))
            or ((global_config.get("asana") or {}).get("workspace"))
        )
        if not effective_workspace:
            raise StepFailure(
                2,
                "No asana.workspace found in repo or global config. Set it before running verification.",
            )

        # Step 3.
        team_name = f"loopflow-verify-{int(time.time())}"
        override_team = api.create_team(str(effective_workspace), team_name)
        override_projects = api.list_team_projects(override_team)
        repo_projects = api.list_team_projects(str(repo_team))

        # Step 4.
        global_config = load_yaml(global_path)
        repo_overrides = global_config.setdefault("repos", {})
        if not isinstance(repo_overrides, dict):
            raise StepFailure(4, "Global config has a non-mapping repos: block.")
        repo_overrides[str(repo)] = {"asana": {"team": override_team}}
        write_yaml(global_path, global_config)

        # Step 5.
        override_stdout = run_pm_list(repo)
        assert_pm_list_matches(5, override_stdout, override_projects)
        if any(project.gid in override_stdout for project in repo_projects):
            raise StepFailure(5, "override output still mentions projects from the repo team")

        # Step 6.
        global_config = remove_repo_override(load_yaml(global_path), str(repo))
        write_yaml(global_path, global_config)

        # Step 7.
        repo_stdout = run_pm_list(repo)
        assert_pm_list_matches(7, repo_stdout, repo_projects)
        if override_team in repo_stdout:
            raise StepFailure(7, "repo-team output still mentions the override team")

        # Step 8.
        api.delete_team(override_team)
        override_team = None

        # Step 9.
        restore()
        api.close()
        print("verified canonical waves via Asana team override")
        return 0
    except StepFailure as err:
        restore()
        if override_team is not None:
            try:
                api.delete_team(override_team)
            except Exception as cleanup_err:  # pragma: no cover - best-effort cleanup
                print(f"cleanup warning: {cleanup_err}", file=sys.stderr)
        api.close()
        print(str(err), file=sys.stderr)
        return 1
    except Exception as err:
        restore()
        if override_team is not None:
            try:
                api.delete_team(override_team)
            except Exception as cleanup_err:  # pragma: no cover - best-effort cleanup
                print(f"cleanup warning: {cleanup_err}", file=sys.stderr)
        api.close()
        print(f"Unexpected failure: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
