"""Dependency checking and auto-installation."""

import shutil
import subprocess
from pathlib import Path

import typer
import yaml

# Package info for each dependency
DEPS = {
    "wt": {
        "name": "worktrunk",
        "method": "brew",
        "package": "wt",
        "tap": "max-sixty/worktrunk",
    },
    "claude": {
        "name": "Claude Code",
        "method": "npm",
        "package": "@anthropic-ai/claude-code",
    },
    "codex": {
        "name": "Codex CLI",
        "method": "npm",
        "package": "@openai/codex",
    },
    "gemini": {
        "name": "Gemini CLI",
        "method": "npm",
        "package": "@google/gemini-cli",
    },
}


def _check_installed(cmd: str) -> bool:
    """Check if a command is available."""
    return shutil.which(cmd) is not None


def _has_brew() -> bool:
    """Check if Homebrew is available."""
    return shutil.which("brew") is not None


def _has_npm() -> bool:
    """Check if npm is available."""
    return shutil.which("npm") is not None


def _install_with_brew(formula: str, tap: str | None = None) -> bool:
    """Install a formula with Homebrew. Returns True on success."""
    if tap:
        result = subprocess.run(["brew", "install", f"{tap}/{formula}"], check=False)
    else:
        result = subprocess.run(["brew", "install", formula], check=False)
    return result.returncode == 0


def _install_with_npm(package: str) -> bool:
    """Install a package globally with npm. Returns True on success."""
    result = subprocess.run(["npm", "install", "-g", package], check=False)
    return result.returncode == 0


def check_missing(*deps: str) -> list[str]:
    """Return list of deps that are not installed."""
    return [d for d in deps if not _check_installed(d)]


def require_deps(
    *deps: str,
    repo_root: Path | None = None,
    set_agent_model: str | None = None,
) -> None:
    """Ensure dependencies are installed, offering to install if missing.

    deps: One or more of "wt", "claude", "codex", "gemini"
    repo_root: If provided with set_agent_model, configures .lf/config.yaml
    set_agent_model: Agent backend to set as default after install
    """
    missing = check_missing(*deps)
    if not missing:
        return

    # Check prerequisites
    need_brew = any(DEPS[d]["method"] == "brew" for d in missing)
    need_npm = any(DEPS[d]["method"] == "npm" for d in missing)

    if need_brew and not _has_brew():
        typer.echo("Error: Some dependencies require Homebrew", err=True)
        typer.echo("Install Homebrew: https://brew.sh", err=True)
        raise typer.Exit(1)

    if need_npm and not _has_npm():
        typer.echo("Error: Some dependencies require Node.js/npm", err=True)
        typer.echo("Install Node.js: https://nodejs.org", err=True)
        raise typer.Exit(1)

    # Show what's missing and prompt
    missing_names = [DEPS[d]["name"] for d in missing]
    typer.echo(f"Missing: {', '.join(missing_names)}")
    if not typer.confirm("Install now?", default=True):
        typer.echo("Run 'lf init' for full setup", err=True)
        raise typer.Exit(1)

    # Install each missing dep
    for dep in missing:
        info = DEPS[dep]
        typer.echo(f"Installing {info['name']}...")

        if info["method"] == "brew":
            success = _install_with_brew(info["package"], info.get("tap"))
        else:
            success = _install_with_npm(info["package"])

        if not success:
            typer.echo(f"Failed to install {info['name']}", err=True)
            raise typer.Exit(1)

    typer.echo("Dependencies installed")

    # Configure agent model if requested
    if repo_root and set_agent_model:
        _set_agent_model(repo_root, set_agent_model)


def _set_agent_model(repo_root: Path, backend: str) -> None:
    """Set agent_model in .lf/config.yaml."""
    config_path = repo_root / ".lf" / "config.yaml"

    if config_path.exists():
        try:
            data = yaml.safe_load(config_path.read_text()) or {}
        except Exception:
            data = {}
    else:
        config_path.parent.mkdir(parents=True, exist_ok=True)
        data = {}

    data["agent_model"] = backend
    config_path.write_text(yaml.dump(data, default_flow_style=False))
    typer.echo(f"Set {backend} as default agent in .lf/config.yaml")


# Convenience functions for common cases
def require_wt() -> None:
    """Ensure worktrunk is installed."""
    require_deps("wt")


def require_agent(backend: str = "claude", repo_root: Path | None = None) -> None:
    """Ensure an agent CLI is installed. Defaults to claude."""
    require_deps(backend, repo_root=repo_root, set_agent_model=backend if repo_root else None)
