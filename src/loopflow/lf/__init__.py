"""Loopflow CLI: Arrange LLMs to code in harmony."""

import sys
from pathlib import Path
from typing import Optional

import typer
import yaml

from loopflow.lf.config import ConfigError, load_config
from loopflow.lf.context import find_worktree_root, gather_task, list_all_tasks
from loopflow.lf.pipelines import load_pipeline


# =============================================================================
# Built-in task metadata for formatted listing
# =============================================================================

BUILTIN_CATEGORIES: dict[str, list[str]] = {
    "Planning & Design": ["design", "explore", "refine"],
    "Implementation": ["implement", "iterate", "expand", "reduce"],
    "Quality": ["review", "polish", "debug"],
    "Git": ["commit", "rebase"],
}

BUILTIN_DESCRIPTIONS: dict[str, str] = {
    "design": "Plan what to build",
    "explore": "Investigate current diff",
    "implement": "Build from design doc",
    "iterate": "Improve code on branch",
    "expand": "Explore ambitious extensions",
    "reduce": "Simplify while preserving behavior",
    "review": "Assess code, write verdict",
    "polish": "Fix issues, run tests",
    "debug": "Fix errors from clipboard",
    "commit": "Commit with generated message",
    "rebase": "Rebase onto main",
    "refine": "Iteratively refine text",
}


def _use_color() -> bool:
    """Check if we should use colored output."""
    return sys.stdout.isatty()


# ANSI color codes
def _colors() -> dict[str, str]:
    if not _use_color():
        return {"cyan": "", "bold": "", "dim": "", "yellow": "", "green": "", "reset": ""}
    return {
        "cyan": "\033[36m",
        "bold": "\033[1m",
        "dim": "\033[90m",
        "yellow": "\033[33m",
        "green": "\033[32m",
        "reset": "\033[0m",
    }

app = typer.Typer(
    name="lf",
    help="Arrange LLMs to code in harmony.",
    no_args_is_help=False,
)

# Import and register subcommands
from loopflow.lf import run as run_module

# Register top-level commands
app.command(context_settings={"allow_extra_args": True, "allow_interspersed_args": True})(run_module.run)
app.command()(run_module.inline)
app.command()(run_module.cp)
app.command()(run_module.add)
app.command(name="pipeline")(run_module.pipeline)


def _parse_frontmatter(content: str) -> dict:
    """Extract frontmatter fields from task content."""
    if not content.startswith("---"):
        return {}
    try:
        _, fm, _ = content.split("---", 2)
        return yaml.safe_load(fm) or {}
    except Exception:
        return {}


def _get_task_info(repo_root: Path | None, name: str, config=None) -> dict:
    """Get task metadata for display."""
    task = gather_task(repo_root, name, config)
    if task and task.content:
        return _parse_frontmatter(task.content)
    return {}


def _format_task_list() -> str:
    """Format tasks and pipelines with colors and categories."""
    c = _colors()
    repo_root = find_worktree_root()
    config = load_config(repo_root) if repo_root else None

    user_tasks, global_tasks, builtin_only, external_skills = list_all_tasks(repo_root, config)
    user_task_set = set(user_tasks)
    global_task_set = set(global_tasks)
    all_known_tasks = user_task_set | global_task_set | set(builtin_only)

    lines = []

    # Pipelines section
    if config and config.pipelines:
        lines.append(f"{c['cyan']}{c['bold']}PIPELINES{c['reset']}")
        for name in sorted(config.pipelines.keys()):
            p = config.pipelines[name]
            chain = f" {c['dim']}→{c['reset']} ".join(p.tasks) if p.tasks else ""
            lines.append(f"  {c['bold']}{name:<14}{c['reset']} {c['dim']}{chain}{c['reset']}")
        lines.append("")

    # Tasks section
    lines.append(f"{c['cyan']}{c['bold']}TASKS{c['reset']}")
    lines.append("")

    # Built-ins by category
    for category, task_names in BUILTIN_CATEGORIES.items():
        category_tasks = [t for t in task_names if t in all_known_tasks]
        if not category_tasks:
            continue

        lines.append(f"{c['dim']}{category}{c['reset']}")
        for name in category_tasks:
            desc = BUILTIN_DESCRIPTIONS.get(name, "")
            info = _get_task_info(repo_root, name, config)
            badge = f"  {c['yellow']}interactive{c['reset']}" if info.get("interactive") else ""
            customized = f" {c['dim']}(customized){c['reset']}" if name in user_task_set else ""
            lines.append(f"  {c['bold']}{name:<14}{c['reset']} {c['dim']}{desc:<34}{c['reset']}{badge}{customized}")
        lines.append("")

    # Custom tasks (user-defined, not overriding builtins)
    custom = [t for t in user_tasks if t not in BUILTIN_DESCRIPTIONS]
    if custom:
        lines.append(f"{c['green']}Custom{c['reset']}")
        for name in sorted(custom):
            info = _get_task_info(repo_root, name, config)
            # Try to get a description from produces or just leave blank
            desc = ""
            if info.get("produces"):
                desc = str(info["produces"])[:34]
            badge = f"  {c['yellow']}interactive{c['reset']}" if info.get("interactive") else ""
            lines.append(f"  {c['bold']}{name:<14}{c['reset']} {c['dim']}{desc:<34}{c['reset']}{badge}")
        lines.append("")

    # Global tasks (e.g., ~/.claude/commands/rams.md)
    if global_tasks:
        lines.append(f"{c['green']}Global{c['reset']}")
        for name in sorted(global_tasks):
            info = _get_task_info(repo_root, name, config)
            desc = ""
            if info.get("produces"):
                desc = str(info["produces"])[:34]
            badge = f"  {c['yellow']}interactive{c['reset']}" if info.get("interactive") else ""
            lines.append(f"  {c['bold']}{name:<14}{c['reset']} {c['dim']}{desc:<34}{c['reset']}{badge}")
        lines.append("")

    # External skills section
    if external_skills:
        # Group by source
        by_source: dict[str, list[str]] = {}
        for prefixed_name, source_name in external_skills:
            by_source.setdefault(source_name, []).append(prefixed_name)

        lines.append(f"{c['cyan']}{c['bold']}EXTERNAL SKILLS{c['reset']}")
        lines.append("")
        for source_name, skill_names in sorted(by_source.items()):
            lines.append(f"{c['dim']}{source_name}{c['reset']}")
            for name in skill_names:
                lines.append(f"  {c['bold']}{name:<20}{c['reset']}")
            lines.append("")

    # Footer
    lines.append(f"{c['dim']}Built-ins work anywhere. Run lf <task> or lf <task>: args{c['reset']}")

    return "\n".join(lines)


def _list_tasks() -> None:
    """Print formatted task list."""
    typer.echo(_format_task_list())


def main():
    """Entry point that supports 'lf <task>' and 'lf <pipeline>' shorthand."""
    known_commands = {
        "run",
        "inline",
        "cp",
        "add",
        "pipeline",
        "loop",
        "--help",
        "-h",
    }

    try:
        # Handle --list / -l flag: show formatted task list
        if "--list" in sys.argv or "-l" in sys.argv:
            _list_tasks()
            raise SystemExit(0)

        # Handle 'lf' with no arguments: launch interactive claude
        if len(sys.argv) == 1:
            sys.argv = ["lf", "run", "--interactive"]

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

                # gather_task now includes builtins and external skills
                has_task = gather_task(repo_root, name, config) is not None if repo_root else False
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
                    typer.echo(f"Run 'lf --list' to see available tasks.", err=True)
                    raise SystemExit(1)

        app()
    except ConfigError as e:
        typer.echo(f"Error: {e}", err=True)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
