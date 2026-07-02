#!/usr/bin/env python3
"""Full PM reset: delete loopflow-managed projects, clear IDs, re-init.

One command to wipe PM state and rebuild it from ``wave/``. Use after a wave
reorg, or any time PM drift has accumulated past the point of incremental
repair.

Usage:
    uv run python scripts/pm_reset.py --provider asana [--dry-run] [--yes]

Flow:
    1. Find every ``wave/<name>/goal.md`` with a configured
       ``pm.<provider>_project`` field.
    2. DELETE each project via the provider API.
    3. Clear the project ID from goal frontmatter.
    4. Run ``lf op pm init --all`` to bootstrap fresh projects.

There is no local roadmap mirror to repopulate — waves read and write their
live roadmap via `lf op roadmap` using the `roadmap:` handle in GOAL.md.

Only Asana is implemented at the moment. Linear/Notion raise on --provider.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

import httpx
import yaml

ASANA_API = "https://app.asana.com/api/1.0"


def _split_frontmatter(content: str) -> tuple[dict, str]:
    if not content.startswith("---"):
        return {}, content
    parts = content.split("---", 2)
    if len(parts) != 3:
        return {}, content
    frontmatter = yaml.safe_load(parts[1]) or {}
    body = parts[2].removeprefix("\n")
    if not isinstance(frontmatter, dict):
        return {}, body
    return frontmatter, body


def _render_goal(frontmatter: dict, body: str) -> str:
    rendered = yaml.safe_dump(frontmatter, sort_keys=False)
    return f"---\n{rendered}---\n{body}"


def find_wave_projects(repo: Path, provider: str) -> list[tuple[Path, str, str]]:
    """Return [(goal_path, wave_name, project_id)] for configured waves."""
    field = f"{provider}_project"
    out: list[tuple[Path, str, str]] = []
    for goal_path in sorted((repo / "wave").glob("*/goal.md")):
        try:
            data, _ = _split_frontmatter(goal_path.read_text())
        except yaml.YAMLError:
            continue
        project_id = (data.get("pm") or {}).get(field)
        if project_id:
            out.append((goal_path, goal_path.parent.name, str(project_id)))
    return out


def load_asana_token(credentials_dir: Path) -> str:
    path = credentials_dir / "asana.json"
    if not path.exists():
        sys.exit(f"No Asana credentials at {path}. Run `lf op auth asana` first.")
    token = json.loads(path.read_text()).get("access_token")
    if not token:
        sys.exit(f"No access_token in {path}")
    return token


def delete_asana_project(token: str, project_id: str, api_base: str = ASANA_API) -> None:
    headers = {"Authorization": f"Bearer {token}"}
    response = httpx.delete(f"{api_base}/projects/{project_id}", headers=headers, timeout=30)
    if response.status_code == 404:
        return
    if response.status_code >= 300:
        raise RuntimeError(
            f"Asana delete failed for {project_id}: {response.status_code} {response.text}"
        )


def clear_project_id(goal_path: Path, provider: str) -> None:
    data, body = _split_frontmatter(goal_path.read_text())
    field = f"{provider}_project"
    pm = data.get("pm") or {}
    if field in pm:
        pm.pop(field)
        if pm:
            data["pm"] = pm
        else:
            data.pop("pm", None)
    goal_path.write_text(_render_goal(data, body))


def run_cmd(cmd: list[str], dry_run: bool, cwd: Path | None = None) -> None:
    print(f"  $ {' '.join(cmd)}")
    if dry_run:
        return
    result = subprocess.run(cmd, check=False, cwd=cwd)
    if result.returncode != 0:
        raise RuntimeError(f"{cmd[0]} failed ({result.returncode})")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--provider", choices=["asana"], default="asana")
    parser.add_argument("--dry-run", action="store_true", help="Print plan, do nothing")
    parser.add_argument("--yes", action="store_true", help="Skip confirmation")
    parser.add_argument("--repo", default=".", help="Repo root (default: cwd)")
    parser.add_argument(
        "--credentials",
        default=str(Path.home() / ".lf/credentials"),
        help="Credentials directory (default: ~/.lf/credentials)",
    )
    parser.add_argument("--skip-init", action="store_true", help="Skip `lf op pm init --all`")
    args = parser.parse_args()

    repo = Path(args.repo).resolve()
    credentials_dir = Path(args.credentials)

    projects = find_wave_projects(repo, args.provider)
    if projects:
        print(f"Found {len(projects)} {args.provider} project(s) to delete:")
        for _, wave, pid in projects:
            print(f"  {wave}: {pid}")
    else:
        print(f"No waves with configured {args.provider}_project — nothing to delete.")

    if projects and not args.yes and not args.dry_run:
        prompt = "\nThis will DELETE every listed project. Proceed? [y/N] "
        if input(prompt).strip().lower() != "y":
            print("Aborted.")
            return 1

    if projects:
        token = load_asana_token(credentials_dir) if not args.dry_run else ""
        print()
        for goal_path, wave, project_id in projects:
            print(f"Deleting {wave} ({project_id})...")
            if not args.dry_run:
                delete_asana_project(token, project_id)
                clear_project_id(goal_path, args.provider)

    if not args.skip_init:
        print("\nRe-initializing from wave/...")
        run_cmd(["lf", "op", "pm", "init", "--all"], args.dry_run, cwd=repo)

    print("\nDone.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
