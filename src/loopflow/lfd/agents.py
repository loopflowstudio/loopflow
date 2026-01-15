"""Agent loading and spawning for lfd daemon."""

import os
import re
import subprocess
import sys
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from loopflow.lfd.db import get_latest_run, save_run, update_run_status
from loopflow.lfd.models import AgentSpec, AgentRun, AgentStatus, TriggerSpec, TriggerKind, MergeStrategy
from loopflow.lfd.naming import agent_branch_name, agent_worktree_path
from loopflow.lfd.process import is_process_running
from loopflow.lfd.triggers import should_trigger
from loopflow.logging import get_log_dir
from loopflow.worktrees import WorktreeError, list_all

AGENTS_DIR = Path.home() / ".lf" / "agents"
_FRONTMATTER_PATTERN = re.compile(r"^---\s*\n(.*?)\n---\s*\n?", re.DOTALL)


def list_agents(agents_dir: Path | None = None) -> list[AgentSpec]:
    """Load agent specs from ~/.lf/agents/*.md files."""
    if agents_dir is None:
        agents_dir = AGENTS_DIR

    if not agents_dir.exists():
        return []

    agents = []
    for path in sorted(agents_dir.glob("*.md")):
        agent = _parse_agent_file(path)
        if agent:
            agents.append(agent)

    return agents


def get_agent(name: str, agents_dir: Path | None = None) -> AgentSpec | None:
    """Get a specific agent by name."""
    if agents_dir is None:
        agents_dir = AGENTS_DIR

    path = agents_dir / f"{name}.md"
    return _parse_agent_file(path)


def _parse_agent_file(path: Path) -> AgentSpec | None:
    """Parse an agent markdown file."""
    if not path.exists() or path.suffix != ".md":
        return None

    text = path.read_text()
    match = _FRONTMATTER_PATTERN.match(text)
    if not match:
        return None

    frontmatter = match.group(1)
    prompt = text[match.end():].strip()
    config = _parse_yaml_frontmatter(frontmatter)

    if not config.get("repo") or not config.get("pipeline"):
        return None

    trigger = _parse_trigger(config)

    # Parse goal path relative to repo
    goal = None
    if config.get("goal"):
        goal = Path(config["goal"])

    # Parse merge strategy
    merge_str = config.get("merge", "pr")
    merge_strategy = MergeStrategy(merge_str)

    # Pipeline can be a string (name) or list (inline tasks)
    pipeline = config["pipeline"]
    if isinstance(pipeline, list):
        # Legacy format: inline list of tasks
        pipeline = ",".join(pipeline)

    return AgentSpec(
        name=path.stem,
        repo=Path(config["repo"]).expanduser(),
        pipeline=pipeline,
        trigger=trigger,
        context=config.get("context", []),
        prompt=prompt,
        emoji=config.get("emoji", ""),
        goal=goal,
        merge_strategy=merge_strategy,
    )


def _parse_trigger(config: dict) -> TriggerSpec:
    """Parse trigger configuration from frontmatter."""
    trigger_val = config.get("trigger", "manual")

    # Handle cron syntax: cron("0 9 * * *", grace: 120)
    if isinstance(trigger_val, str) and trigger_val.startswith("cron("):
        cron_match = re.match(r'cron\("([^"]+)"(?:,\s*grace:\s*(\d+))?\)', trigger_val)
        if cron_match:
            cron_expr = cron_match.group(1)
            grace = int(cron_match.group(2)) if cron_match.group(2) else 60
            return TriggerSpec(
                kind=TriggerKind.CRON,
                cron=cron_expr,
                grace_minutes=grace,
            )

    # Simple trigger kinds
    if isinstance(trigger_val, str):
        try:
            kind = TriggerKind(trigger_val)
        except ValueError:
            kind = TriggerKind.MANUAL
        return TriggerSpec(
            kind=kind,
            interval_seconds=config.get("interval"),
            cron=config.get("cron"),
            grace_minutes=config.get("grace", 60),
        )

    return TriggerSpec()


def _parse_yaml_frontmatter(text: str) -> dict:
    """Parse simple YAML frontmatter."""
    result: dict = {}
    current_key = None

    for line in text.split("\n"):
        line = line.rstrip()
        if not line or line.startswith("#"):
            continue

        if line.startswith("  - ") and current_key:
            if current_key not in result:
                result[current_key] = []
            result[current_key].append(line[4:].strip())
            continue

        if ":" in line:
            key, _, value = line.partition(":")
            key = key.strip()
            value = value.strip()
            current_key = key

            if not value:
                continue

            if value.startswith("[") and value.endswith("]"):
                items = value[1:-1].split(",")
                result[key] = [item.strip() for item in items if item.strip()]
            elif value.lower() in ("true", "yes"):
                result[key] = True
            elif value.lower() in ("false", "no"):
                result[key] = False
            elif value.isdigit():
                result[key] = int(value)
            else:
                result[key] = value

    return result


def get_agent_file_path(name: str, agents_dir: Path | None = None) -> Path | None:
    """Get the path to an agent file if it exists."""
    if agents_dir is None:
        agents_dir = AGENTS_DIR

    path = agents_dir / f"{name}.md"
    return path if path.exists() else None


def create_agent_file(
    name: str,
    repo: Path,
    pipeline: str,
    trigger: str = "manual",
    context: list[str] | None = None,
    prompt: str = "",
    interval_seconds: int | None = None,
    emoji: str = "",
    goal: Path | None = None,
    merge: str = "pr",
    cron: str | None = None,
    grace_minutes: int | None = None,
    agents_dir: Path | None = None,
) -> Path:
    """Create a new agent markdown file."""
    if agents_dir is None:
        agents_dir = AGENTS_DIR

    agents_dir.mkdir(parents=True, exist_ok=True)
    path = agents_dir / f"{name}.md"

    lines = ["---"]
    if emoji:
        lines.append(f"emoji: {emoji}")
    lines.append(f"repo: {repo}")
    if goal:
        lines.append(f"goal: {goal}")
    lines.append(f"pipeline: {pipeline}")
    lines.append(f"merge: {merge}")

    # Build trigger line
    if trigger == "cron" and cron:
        if grace_minutes and grace_minutes != 60:
            lines.append(f'trigger: cron("{cron}", grace: {grace_minutes})')
        else:
            lines.append(f'trigger: cron("{cron}")')
    else:
        lines.append(f"trigger: {trigger}")
        if interval_seconds and trigger == "interval":
            lines.append(f"interval: {interval_seconds}")

    if context:
        lines.append(f"context: [{', '.join(context)}]")
    lines.append("---")
    lines.append("")
    lines.append(prompt or "Describe what this agent should do.")

    path.write_text("\n".join(lines) + "\n")
    return path


def delete_agent_file(name: str, agents_dir: Path | None = None) -> bool:
    """Delete an agent markdown file."""
    if agents_dir is None:
        agents_dir = AGENTS_DIR

    path = agents_dir / f"{name}.md"
    if path.exists():
        path.unlink()
        return True
    return False


def get_worktree_path(agent: AgentSpec, iteration: int | None = None) -> Path | None:
    """Get the path to an agent's worktree, if it exists.

    If iteration is provided, looks for that specific iteration.
    Otherwise, finds the latest matching worktree.
    """
    try:
        existing = list_all(agent.repo)
        matching = []
        for wt in existing:
            # Check if branch matches agent pattern
            if agent.emoji and wt.branch.startswith(f"{agent.emoji}/{agent.name}/"):
                matching.append(wt)
            elif not agent.emoji and wt.branch.startswith(f"agent/{agent.name}/"):
                matching.append(wt)

        if not matching:
            return None

        if iteration is not None:
            expected_branch = agent_branch_name(agent, iteration)
            for wt in matching:
                if wt.branch == expected_branch:
                    return wt.path
            return None

        # Return the most recent (highest iteration number)
        matching.sort(key=lambda wt: wt.branch, reverse=True)
        return matching[0].path
    except WorktreeError:
        pass

    return None


@dataclass
class StartResult:
    success: bool
    pid: int | None = None
    error: str | None = None


async def start_agent(name: str) -> StartResult:
    """Start an agent running."""
    agent = get_agent(name)
    if not agent:
        return StartResult(success=False, error=f"Agent '{name}' not found")

    # Check if already running
    latest = get_latest_run(name)
    if latest and latest.status == AgentStatus.RUNNING:
        if latest.pid and is_process_running(latest.pid):
            return StartResult(success=False, error=f"Agent already running (PID {latest.pid})")

    iteration = (latest.iteration + 1) if latest else 1

    # Create run record with emoji
    run = AgentRun(
        id=str(uuid.uuid4()),
        agent_name=name,
        status=AgentStatus.RUNNING,
        started_at=datetime.now(),
        iteration=iteration,
        emoji=agent.emoji,
    )

    # Calculate worktree path for this iteration
    wt_path = agent_worktree_path(agent.repo, agent, iteration)
    run.worktree = str(wt_path)

    # Spawn agent process
    log_dir = get_log_dir(agent.repo)
    log_path = log_dir / f"agent-{name}.log"

    cmd = [
        sys.executable,
        "-m",
        "loopflow.lfd.agent_runner",
        "--agent-name",
        name,
        "--run-id",
        run.id,
        "--repo-root",
        str(agent.repo),
    ]

    with log_path.open("a") as log_file:
        process = subprocess.Popen(
            cmd,
            stdout=subprocess.DEVNULL,
            stderr=log_file,
            start_new_session=True,
        )

    run.pid = process.pid
    save_run(run)

    return StartResult(success=True, pid=process.pid)


def stop_agent(name: str) -> bool:
    """Stop a running agent."""
    latest = get_latest_run(name)
    if not latest or latest.status != AgentStatus.RUNNING:
        return False

    if latest.pid:
        try:
            os.kill(latest.pid, 15)  # SIGTERM
        except OSError:
            pass

    update_run_status(latest.id, AgentStatus.STOPPED)
    return True


async def check_and_run_triggers() -> None:
    """Check all agents and run those whose triggers are met."""
    for agent in list_agents():
        if agent.trigger.kind == TriggerKind.MANUAL:
            continue

        latest = get_latest_run(agent.name)
        if latest and latest.status == AgentStatus.RUNNING:
            continue

        if should_trigger(agent, latest):
            await start_agent(agent.name)
