"""Loopflow CLI: Arrange LLMs to code in harmony."""

import sys
from pathlib import Path
from typing import Optional

import typer
import yaml

from loopflow.config import ConfigError, load_config
from loopflow.context import find_worktree_root, gather_task, list_all_tasks, _get_builtin_task
from loopflow.init_check import check_init_status
from loopflow.lfd.pipelines import load_pipeline

app = typer.Typer(
    name="lf",
    help="Arrange LLMs to code in harmony.",
    no_args_is_help=False,
)

# Import and register subcommands
from loopflow.cli import run as run_module

# Register top-level commands
app.command(context_settings={"allow_extra_args": True, "allow_interspersed_args": True})(run_module.run)
app.command()(run_module.inline)
app.command(name="pipeline")(run_module.pipeline)
app.command()(run_module.cp)


def _get_task_source(repo_root: Path | None, name: str) -> str:
    """Return source location: .claude, .lf, or builtin."""
    if repo_root:
        if (repo_root / ".claude" / "commands" / f"{name}.md").exists():
            return ".claude"
        lf_dir = repo_root / ".lf"
        if lf_dir.exists():
            for p in lf_dir.iterdir():
                if p.is_file() and (p.stem == name or p.name == name):
                    return ".lf"
    return "builtin"


def _parse_frontmatter(content: str) -> dict:
    """Extract frontmatter fields from task content."""
    if not content.startswith("---"):
        return {}
    try:
        _, fm, _ = content.split("---", 2)
        return yaml.safe_load(fm) or {}
    except Exception:
        return {}


def _get_task_info(repo_root: Path | None, name: str) -> dict:
    """Get task metadata for display."""
    info = {"name": name, "source": _get_task_source(repo_root, name)}

    # Find the actual file to read frontmatter
    content = None
    if repo_root:
        # Check .claude/commands first
        claude_path = repo_root / ".claude" / "commands" / f"{name}.md"
        if claude_path.exists():
            content = claude_path.read_text()
        else:
            # Check .lf
            lf_dir = repo_root / ".lf"
            if lf_dir.exists():
                for p in lf_dir.iterdir():
                    if p.is_file() and (p.stem == name or p.name == name):
                        content = p.read_text()
                        break

    # Fall back to builtin
    if content is None:
        builtin = _get_builtin_task(name)
        if builtin:
            content = builtin.read_text()

    if content:
        fm = _parse_frontmatter(content)
        info.update(fm)

    return info


def _list_tasks() -> None:
    """List available tasks and pipelines."""
    repo_root = find_worktree_root()
    config = load_config(repo_root) if repo_root else None

    user_tasks, builtin_only = list_all_tasks(repo_root)
    pipelines = list(config.pipelines.keys()) if config else []

    # Show pipelines
    if pipelines:
        typer.echo("Pipelines:")
        for name in sorted(pipelines):
            p = config.pipelines[name]
            tasks_str = " → ".join(p.tasks) if p.tasks else ""
            typer.echo(f"  {name:<16} {tasks_str}")
        typer.echo()

    # Gather task info
    all_tasks = []
    for name in user_tasks:
        info = _get_task_info(repo_root, name)
        info["builtin"] = False
        all_tasks.append(info)
    for name in builtin_only:
        info = _get_task_info(repo_root, name)
        info["builtin"] = True
        all_tasks.append(info)

    if not all_tasks:
        typer.echo("No tasks found.")
        typer.echo("Run: lfops init")
        return

    def format_task(t: dict, show_source: bool = False) -> str:
        """Format a task for display."""
        parts = [f"  {t['name']:<14}"]
        if show_source:
            parts.append(f" {t['source']:<8}")

        meta = []
        if t.get("interactive"):
            meta.append("i")
        if t.get("requires"):
            meta.append(f"← {t['requires']}")
        if t.get("produces"):
            meta.append(f"→ {t['produces']}")

        if meta:
            parts.append(f"  {' '.join(meta)}")
        return "".join(parts)

    # Display custom tasks
    custom = [t for t in all_tasks if not t["builtin"]]
    if custom:
        typer.echo("Tasks:")
        for t in custom:
            typer.echo(format_task(t, show_source=True))
        typer.echo()

    # Display builtins
    builtins = [t for t in all_tasks if t["builtin"]]
    if builtins:
        typer.echo("Built-in:")
        for t in builtins:
            typer.echo(format_task(t, show_source=False))


def main():
    """Entry point that supports 'lf <task>' and 'lf <pipeline>' shorthand."""
    known_commands = {
        "run",
        "pipeline",
        "inline",
        "cp",
        "--help",
        "-h",
    }

    try:
        # Handle 'lf' with no arguments: list available tasks
        if len(sys.argv) == 1:
            _list_tasks()
            raise SystemExit(0)

        if len(sys.argv) > 1:
            first_arg = sys.argv[1]

            # Inline prompt: lf : "prompt"
            if first_arg == ":":
                sys.argv.pop(1)
                sys.argv.insert(1, "inline")
            elif first_arg not in known_commands:
                # Handle colon suffix: "lf implement: add logout" -> "lf implement add logout"
                if first_arg.endswith(":"):
                    sys.argv[1] = first_arg[:-1]
                name = sys.argv[1]
                repo_root = find_worktree_root()
                config = load_config(repo_root) if repo_root else None

                # Check for pipeline in config.yaml or .lf/pipelines/
                has_config_pipeline = config and name in config.pipelines
                has_file_pipeline = repo_root and load_pipeline(name, repo_root) is not None
                has_pipeline = has_config_pipeline or has_file_pipeline

                # gather_task now includes builtins
                has_task = gather_task(repo_root, name) is not None if repo_root else False
                # Also check builtin even without repo_root
                if not has_task and not repo_root:
                    has_task = _get_builtin_task(name) is not None

                if has_pipeline and has_task:
                    typer.echo(f"Error: '{name}' exists as both a pipeline and a task", err=True)
                    typer.echo(f"  Pipeline: defined in .lf/config.yaml", err=True)
                    typer.echo(f"  Task: .claude/commands/{name}.md or .lf/{name}.*", err=True)
                    typer.echo(f"Remove one to resolve the conflict.", err=True)
                    raise SystemExit(1)

                if has_pipeline:
                    sys.argv.insert(1, "pipeline")
                elif has_task:
                    sys.argv.insert(1, "run")
                else:
                    # Task not found
                    typer.echo(f"No task or pipeline named '{name}'", err=True)
                    typer.echo(f"Run 'lf' to see available tasks.", err=True)
                    raise SystemExit(1)

        app()
    except ConfigError as e:
        typer.echo(f"Error: {e}", err=True)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
