"""Loopflow CLI: Arrange LLMs to code in harmony."""

import sys
from pathlib import Path

import typer
import yaml

from loopflow.lf import step as step_module
from loopflow.lf.config import ConfigError, load_config
from loopflow.lf.context import find_worktree_root, gather_step, list_all_steps
from loopflow.lf.flows import list_flows, load_flow

# =============================================================================
# Built-in step metadata for formatted listing
# =============================================================================

BUILTIN_CATEGORIES: dict[str, list[str]] = {
    "Setup": ["init"],
    "Planning & Design": ["design", "explore", "refine"],
    "Implementation": ["implement", "iterate", "expand", "reduce"],
    "Quality": ["review", "polish", "lint", "debug"],
    "Git": ["commit", "rebase"],
}

BUILTIN_DESCRIPTIONS: dict[str, str] = {
    "init": "Set up loopflow in this repo",
    "design": "Plan what to build",
    "explore": "Investigate current diff",
    "implement": "Build from design doc",
    "iterate": "Improve code on branch",
    "expand": "Explore ambitious extensions",
    "reduce": "Simplify while preserving behavior",
    "review": "Assess code, write verdict",
    "polish": "Fix issues, run tests",
    "lint": "Run linter, fix issues",
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

# Register top-level commands
# lf is a prompt launcher - every command launches a prompt
_run_ctx = {"allow_extra_args": True, "allow_interspersed_args": True}
app.command(context_settings=_run_ctx)(step_module.run)
app.command()(step_module.inline)
app.command(name="flow")(step_module.flow)


def _parse_frontmatter(content: str) -> dict:
    """Extract frontmatter fields from step content."""
    if not content.startswith("---"):
        return {}
    try:
        _, fm, _ = content.split("---", 2)
        return yaml.safe_load(fm) or {}
    except Exception:
        return {}


def _get_step_info(repo_root: Path | None, name: str, config=None) -> dict:
    """Get step metadata for display."""
    step = gather_step(repo_root, name, config)
    if step and step.content:
        return _parse_frontmatter(step.content)
    return {}


def _format_step_list() -> str:
    """Format steps and pipelines with colors and categories."""
    c = _colors()
    repo_root = find_worktree_root()
    config = load_config(repo_root) if repo_root else None

    user_steps, global_steps, builtin_only, external_skills = list_all_steps(repo_root, config)
    user_step_set = set(user_steps)
    global_step_set = set(global_steps)
    all_known_steps = user_step_set | global_step_set | set(builtin_only)

    lines = []

    # Flows section
    if repo_root:
        flows = list_flows(repo_root)
    else:
        flows = []

    if flows:
        lines.append(f"{c['cyan']}{c['bold']}FLOWS{c['reset']}")
        for flow in sorted(flows, key=lambda f: f.name):
            chain = f" {c['dim']}→{c['reset']} ".join(step.step for step in flow.steps if step.step)
            lines.append(f"  {c['bold']}{flow.name:<14}{c['reset']} {c['dim']}{chain}{c['reset']}")
        lines.append("")

    # Steps section
    lines.append(f"{c['cyan']}{c['bold']}STEPS{c['reset']}")
    lines.append("")

    # Built-ins by category
    for category, step_names in BUILTIN_CATEGORIES.items():
        category_steps = [t for t in step_names if t in all_known_steps]
        if not category_steps:
            continue

        lines.append(f"{c['dim']}{category}{c['reset']}")
        for name in category_steps:
            desc = BUILTIN_DESCRIPTIONS.get(name, "")
            info = _get_step_info(repo_root, name, config)
            badge = f"  {c['yellow']}interactive{c['reset']}" if info.get("interactive") else ""
            custom_tag = f" {c['dim']}(customized){c['reset']}" if name in user_step_set else ""
            lines.append(
                f"  {c['bold']}{name:<14}{c['reset']} {c['dim']}{desc:<34}{c['reset']}"
                f"{badge}{custom_tag}"
            )
        lines.append("")

    # Custom steps (user-defined, not overriding builtins)
    custom = [t for t in user_steps if t not in BUILTIN_DESCRIPTIONS]
    if custom:
        lines.append(f"{c['green']}Custom{c['reset']}")
        for name in sorted(custom):
            info = _get_step_info(repo_root, name, config)
            # Try to get a description from produces or just leave blank
            desc = ""
            if info.get("produces"):
                desc = str(info["produces"])[:34]
            badge = f"  {c['yellow']}interactive{c['reset']}" if info.get("interactive") else ""
            lines.append(
                f"  {c['bold']}{name:<14}{c['reset']} {c['dim']}{desc:<34}{c['reset']}{badge}"
            )
        lines.append("")

    # Global steps (user-installed at ~/.claude/commands/)
    if global_steps:
        lines.append(f"{c['green']}Global{c['reset']}")
        for name in sorted(global_steps):
            info = _get_step_info(repo_root, name, config)
            desc = ""
            if info.get("produces"):
                desc = str(info["produces"])[:34]
            badge = f"  {c['yellow']}interactive{c['reset']}" if info.get("interactive") else ""
            lines.append(
                f"  {c['bold']}{name:<14}{c['reset']} {c['dim']}{desc:<34}{c['reset']}{badge}"
            )
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
    lines.append(f"{c['dim']}Built-ins work anywhere. Run lf <step> or lf <step>: args{c['reset']}")

    return "\n".join(lines)


def _list_steps() -> None:
    """Print formatted step list."""
    typer.echo(_format_step_list())


def main():
    """Entry point that supports 'lf <step>' and 'lf <pipeline>' shorthand."""
    # lf commands - all launch prompts
    known_commands = {
        "run",
        "inline",
        "flow",
        "--help",
        "-h",
    }

    try:
        # Handle --list / -l flag: show formatted step list
        if "--list" in sys.argv or "-l" in sys.argv:
            _list_steps()
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
            # Flags without step: lf -m codex → lf run -m codex
            elif first_arg.startswith("-") and first_arg not in known_commands:
                sys.argv.insert(1, "run")
            elif first_arg not in known_commands:
                # Handle colon suffix: "lf implement: add logout" -> "lf implement add logout"
                if first_arg.endswith(":"):
                    sys.argv[1] = first_arg[:-1]
                name = sys.argv[1]
                repo_root = find_worktree_root()
                config = load_config(repo_root) if repo_root else None

                # Check for flow in .lf/flows/
                has_flow = repo_root and load_flow(name, repo_root) is not None

                # gather_step handles builtins and external skills even without repo_root
                has_step = gather_step(repo_root, name, config) is not None

                if has_flow and has_step:
                    typer.echo(f"Error: '{name}' exists as both a flow and a step", err=True)
                    typer.echo("  Flow: defined in .lf/flows/", err=True)
                    typer.echo(f"  Step: .claude/commands/{name}.md or .lf/{name}.*", err=True)
                    typer.echo("Remove one to resolve the conflict.", err=True)
                    raise SystemExit(1)

                if has_flow:
                    sys.argv.insert(1, "flow")
                elif has_step:
                    sys.argv.insert(1, "run")
                else:
                    # Step not found
                    typer.echo(f"No step or flow named '{name}'", err=True)
                    typer.echo("Run 'lf --list' to see available steps.", err=True)
                    raise SystemExit(1)

        app()
    except ConfigError as e:
        typer.echo(f"Error: {e}", err=True)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
