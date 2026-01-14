"""Agent loading and spawning for lfd daemon."""

import os
import re
import subprocess
import sys
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from loopflow.lfd.models import AgentSpec, AgentRun, AgentStatus, TriggerSpec, TriggerKind
from loopflow.lfd.process import is_process_running

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

    trigger_str = config.get("trigger", "manual")
    trigger = TriggerSpec(
        kind=TriggerKind(trigger_str),
        interval_seconds=config.get("interval"),
    )

    return AgentSpec(
        name=path.stem,
        repo=Path(config["repo"]).expanduser(),
        pipeline=config["pipeline"],
        trigger=trigger,
        context=config.get("context", []),
        prompt=prompt,
    )


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


@dataclass
class StartResult:
    success: bool
    pid: int | None = None
    error: str | None = None


async def start_agent(name: str) -> StartResult:
    """Start an agent running."""
    from loopflow.lfd.db import get_latest_run, save_run
    from loopflow.logging import get_log_dir

    agent = get_agent(name)
    if not agent:
        return StartResult(success=False, error=f"Agent '{name}' not found")

    # Check if already running
    latest = get_latest_run(name)
    if latest and latest.status == AgentStatus.RUNNING:
        if latest.pid and is_process_running(latest.pid):
            return StartResult(success=False, error=f"Agent already running (PID {latest.pid})")

    # Create run record
    run = AgentRun(
        id=str(uuid.uuid4()),
        agent_name=name,
        status=AgentStatus.RUNNING,
        started_at=datetime.now(),
        iteration=(latest.iteration + 1) if latest else 1,
    )

    # Spawn agent process
    log_dir = get_log_dir(agent.repo)
    log_path = log_dir / f"agent-{name}.log"

    cmd = [
        sys.executable,
        "-m",
        "loopflow.maestro.agent_runner",
        "--agent-id",
        name,
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
    from loopflow.lfd.db import get_latest_run, update_run_status

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
    from loopflow.lfd.db import get_latest_run
    from loopflow.lfd.triggers import should_trigger

    for agent in list_agents():
        if agent.trigger.kind == TriggerKind.MANUAL:
            continue

        latest = get_latest_run(agent.name)
        if latest and latest.status == AgentStatus.RUNNING:
            continue

        if should_trigger(agent, latest):
            await start_agent(agent.name)
