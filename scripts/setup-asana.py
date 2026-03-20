#!/usr/bin/env python3
"""Set up Asana integration for a wave.

Walks through:
1. Verify ASANA_ACCESS_TOKEN is set
2. List workspaces, auto-select or prompt
3. List projects in workspace, let user pick or create new
4. Write asana config to .lf/config.yaml and wave pm block

Usage:
    uv run python scripts/setup-asana.py pm
"""

import os
import sys
from pathlib import Path

import requests
import yaml

ASANA_BASE = "https://app.asana.com/api/1.0"


def main():
    if len(sys.argv) < 2:
        print("Usage: setup-asana.py <wave-name>")
        sys.exit(1)

    wave_name = sys.argv[1]
    repo_root = find_repo_root()
    wave_yaml = repo_root / "wave" / wave_name / f"{wave_name}.yaml"

    if not wave_yaml.parent.is_dir():
        print(f"Wave directory not found: wave/{wave_name}/")
        sys.exit(1)

    # 1. Token
    token = os.environ.get("ASANA_ACCESS_TOKEN", "").strip()
    if not token:
        print("ASANA_ACCESS_TOKEN not set.")
        print("Get a Personal Access Token from: https://app.asana.com/0/developer-console")
        print("Then: export ASANA_ACCESS_TOKEN=<token>")
        sys.exit(1)

    headers = {"Authorization": f"Bearer {token}"}
    print("Authenticated with Asana.\n")

    # 2. Workspace
    workspaces = asana_get(headers, "/workspaces")
    if not workspaces:
        print("No workspaces found for this token.")
        sys.exit(1)

    if len(workspaces) == 1:
        workspace = workspaces[0]
        print(f"Workspace: {workspace['name']} ({workspace['gid']})")
    else:
        print("Workspaces:")
        for i, ws in enumerate(workspaces, 1):
            print(f"  {i}. {ws['name']} ({ws['gid']})")
        choice = input_choice("Select workspace", len(workspaces))
        workspace = workspaces[choice - 1]

    workspace_gid = workspace["gid"]

    # 3. Project
    projects = asana_get(headers, f"/workspaces/{workspace_gid}/projects")
    print(f"\nProjects in {workspace['name']}:")
    for i, proj in enumerate(projects, 1):
        print(f"  {i}. {proj['name']} ({proj['gid']})")
    print(f"  {len(projects) + 1}. Create new project")

    choice = input_choice("Select project", len(projects) + 1)
    if choice <= len(projects):
        project = projects[choice - 1]
        print(f"\nUsing project: {project['name']}")
    else:
        name = input(f"Project name [{wave_name}]: ").strip() or wave_name
        body = {"data": {"name": name, "workspace": workspace_gid}}
        if workspace.get("is_organization"):
            # Organizations may need a team
            teams = asana_get(headers, f"/organizations/{workspace_gid}/teams")
            if teams:
                print("\nTeams:")
                for i, team in enumerate(teams, 1):
                    print(f"  {i}. {team['name']} ({team['gid']})")
                team_choice = input_choice("Select team for project", len(teams))
                body["data"]["team"] = teams[team_choice - 1]["gid"]
        resp = requests.post(
            f"{ASANA_BASE}/projects",
            headers=headers,
            json=body,
        )
        resp.raise_for_status()
        project = resp.json()["data"]
        print(f"\nCreated project: {project['name']} ({project['gid']})")

    project_gid = project["gid"]

    # 4. Write configs
    # .lf/config.yaml — asana workspace
    lf_config_path = repo_root / ".lf" / "config.yaml"
    lf_config = {}
    if lf_config_path.exists():
        lf_config = yaml.safe_load(lf_config_path.read_text()) or {}

    asana_config = lf_config.get("asana", {})
    asana_config["workspace"] = workspace_gid
    lf_config["asana"] = asana_config
    lf_config_path.parent.mkdir(parents=True, exist_ok=True)
    lf_config_path.write_text(yaml.dump(lf_config, default_flow_style=False))
    print("\nWrote asana.workspace to .lf/config.yaml")

    # wave/<name>/<name>.yaml — pm block
    wave_config = {}
    if wave_yaml.exists():
        wave_config = yaml.safe_load(wave_yaml.read_text()) or {}

    wave_config["pm"] = {"provider": "asana", "project": project_gid}
    wave_yaml.write_text(yaml.dump(wave_config, default_flow_style=False))
    print(f"Wrote pm block to wave/{wave_name}/{wave_name}.yaml")

    print("\nReady. Run:")
    print(f"  lf ops export {wave_name} --dry-run")
    print(f"  lf ops export {wave_name}")


def asana_get(headers: dict, path: str) -> list:
    resp = requests.get(f"{ASANA_BASE}{path}", headers=headers)
    resp.raise_for_status()
    return resp.json().get("data", [])


def input_choice(prompt: str, max_val: int) -> int:
    while True:
        raw = input(f"{prompt} [1-{max_val}]: ").strip()
        try:
            val = int(raw)
            if 1 <= val <= max_val:
                return val
        except ValueError:
            pass
        print(f"  Enter a number between 1 and {max_val}")


def find_repo_root() -> Path:
    path = Path.cwd()
    while path != path.parent:
        if (path / ".git").exists():
            return path
        path = path.parent
    return Path.cwd()


if __name__ == "__main__":
    main()
