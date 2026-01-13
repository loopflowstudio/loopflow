"""Agent loop management CLI commands."""

from pathlib import Path

import typer

from loopflow.context import find_worktree_root
from loopflow.git import find_main_repo
from loopflow.maestro.agent import AgentLoopSpec, AgentStatus, OuterLoopConfig, OuterLoopMode
from loopflow.maestro.agents import (
    get_agent,
    get_agent_by_name,
    list_agents,
    register_agent,
    remove_agent,
    start_agent,
    stop_agent,
)

app = typer.Typer(help="Background agent management.")


def _resolve_agent(identifier: str):
    """Resolve agent by ID prefix or name."""
    # Try by name first
    agent = get_agent_by_name(identifier)
    if agent:
        return agent

    # Try by ID prefix
    agents = list_agents()
    matches = [a for a in agents if a.id.startswith(identifier)]
    if len(matches) == 1:
        return matches[0]
    if len(matches) > 1:
        ids = ", ".join(a.id[:8] for a in matches)
        typer.echo(f"Error: Ambiguous identifier '{identifier}': {ids}", err=True)
        raise typer.Exit(1)

    return None


@app.command()
def register(
    name: str = typer.Argument(help="Agent name"),
    prompt: str = typer.Option(..., "-p", "--prompt", help="Path to prompt file (relative to repo root)"),
    pipeline: str = typer.Option(..., "--pipeline", help="Comma-separated list of tasks"),
    context: str = typer.Option(None, "-x", "--context", help="Comma-separated context paths"),
    outer_loop: str = typer.Option(
        "land-commits",
        "-o",
        "--outer-loop",
        help="Outer loop mode: pr-chain or land-commits",
    ),
):
    """Register a background agent with a prompt file and pipeline.

    Examples:
        lf ops agent register ui-agent -p prompts/ui.md --pipeline design,implement,review
        lf ops agent register docs-agent -p prompts/docs.md --pipeline implement -o pr-chain
    """
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo_root) or repo_root

    # Parse pipeline
    tasks = [t.strip() for t in pipeline.split(",") if t.strip()]
    if not tasks:
        typer.echo("Error: Pipeline must have at least one task", err=True)
        raise typer.Exit(1)

    # Parse context
    context_list = []
    if context:
        context_list = [c.strip() for c in context.split(",") if c.strip()]

    # Parse outer loop mode
    try:
        mode = OuterLoopMode(outer_loop)
    except ValueError:
        typer.echo(f"Error: Invalid outer loop mode: {outer_loop}", err=True)
        typer.echo("Valid modes: pr-chain, land-commits", err=True)
        raise typer.Exit(1)

    # Validate prompt file exists
    prompt_path = Path(prompt)
    if not prompt_path.is_absolute():
        full_path = main_repo / prompt_path
    else:
        full_path = prompt_path

    if not full_path.exists():
        typer.echo(f"Error: Prompt file not found: {full_path}", err=True)
        raise typer.Exit(1)

    spec = AgentLoopSpec(
        name=name,
        prompt_path=prompt_path,
        pipeline=tasks,
        context=context_list,
        outer_loop=OuterLoopConfig(mode=mode),
    )

    agent = register_agent(spec)
    typer.echo(f"Registered agent: {name} ({agent.id[:8]})")
    typer.echo(f"  Pipeline: {' → '.join(tasks)}")
    typer.echo(f"  Outer loop: {mode.value}")


@app.command(name="list")
def list_cmd():
    """List all registered agents."""
    agents = list_agents()

    if not agents:
        typer.echo("No agents registered")
        raise typer.Exit(0)

    typer.echo(f"{'ID':<10} {'NAME':<20} {'STATUS':<10} {'PIPELINE':<30} {'ITER'}")

    for agent in agents:
        pipeline_str = " → ".join(agent.spec.pipeline)
        if len(pipeline_str) > 28:
            pipeline_str = pipeline_str[:25] + "..."
        typer.echo(
            f"{agent.id[:8]:<10} {agent.spec.name:<20} {agent.status.value:<10} {pipeline_str:<30} {agent.iteration}"
        )


@app.command()
def start(
    identifier: str = typer.Argument(help="Agent name or ID prefix"),
    foreground: bool = typer.Option(False, "-f", "--foreground", help="Run in foreground"),
):
    """Start a registered agent.

    Runs one iteration of the agent's pipeline in a new worktree.

    Examples:
        lf ops agent start ui-agent
        lf ops agent start ui-agent -f
    """
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo_root) or repo_root

    agent = _resolve_agent(identifier)
    if not agent:
        typer.echo(f"Error: Agent not found: {identifier}", err=True)
        raise typer.Exit(1)

    if agent.status == AgentStatus.RUNNING:
        typer.echo(f"Agent {agent.spec.name} is already running", err=True)
        raise typer.Exit(1)

    typer.echo(f"Starting agent: {agent.spec.name}")

    success = start_agent(agent.id, main_repo, background=not foreground)
    if success:
        if not foreground:
            typer.echo(f"Agent started in background")
    else:
        typer.echo("Failed to start agent", err=True)
        raise typer.Exit(1)


@app.command()
def stop(
    identifier: str = typer.Argument(help="Agent name or ID prefix"),
):
    """Stop a running agent.

    Examples:
        lf ops agent stop ui-agent
    """
    agent = _resolve_agent(identifier)
    if not agent:
        typer.echo(f"Error: Agent not found: {identifier}", err=True)
        raise typer.Exit(1)

    if agent.status != AgentStatus.RUNNING:
        typer.echo(f"Agent {agent.spec.name} is not running")
        raise typer.Exit(0)

    success = stop_agent(agent.id)
    if success:
        typer.echo(f"Stopped agent: {agent.spec.name}")
    else:
        typer.echo("Failed to stop agent", err=True)
        raise typer.Exit(1)


@app.command()
def remove(
    identifier: str = typer.Argument(help="Agent name or ID prefix"),
):
    """Remove an agent registration.

    Examples:
        lf ops agent remove ui-agent
    """
    agent = _resolve_agent(identifier)
    if not agent:
        typer.echo(f"Error: Agent not found: {identifier}", err=True)
        raise typer.Exit(1)

    success = remove_agent(agent.id)
    if success:
        typer.echo(f"Removed agent: {agent.spec.name}")
    else:
        typer.echo("Failed to remove agent", err=True)
        raise typer.Exit(1)


@app.command()
def show(
    identifier: str = typer.Argument(help="Agent name or ID prefix"),
):
    """Show details of an agent.

    Examples:
        lf ops agent show ui-agent
    """
    agent = _resolve_agent(identifier)
    if not agent:
        typer.echo(f"Error: Agent not found: {identifier}", err=True)
        raise typer.Exit(1)

    typer.echo(f"Agent: {agent.spec.name}")
    typer.echo(f"  ID: {agent.id}")
    typer.echo(f"  Status: {agent.status.value}")
    typer.echo(f"  Prompt: {agent.spec.prompt_path}")
    typer.echo(f"  Pipeline: {' → '.join(agent.spec.pipeline)}")
    typer.echo(f"  Outer loop: {agent.spec.outer_loop.mode.value}")
    typer.echo(f"  Iterations: {agent.iteration}")
    if agent.spec.context:
        typer.echo(f"  Context: {', '.join(agent.spec.context)}")
    if agent.last_run_at:
        typer.echo(f"  Last run: {agent.last_run_at.isoformat()}")
    if agent.current_branch:
        typer.echo(f"  Current branch: {agent.current_branch}")
    if agent.current_worktree:
        typer.echo(f"  Current worktree: {agent.current_worktree}")
