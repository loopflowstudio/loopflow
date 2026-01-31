"""Shell integration for lfops.

Provides directory switching after worktree operations via directive files.
"""

import os
from pathlib import Path
from typing import Annotated

import typer

SHELL_INIT_ZSH = """\
# loopflow shell integration for zsh
#
# Enables directory switching after `lfops wt create`.

if command -v lfops >/dev/null 2>&1; then
    lfops() {
        local directive_file exit_code=0
        directive_file="$(mktemp)"

        LOOPFLOW_DIRECTIVE_FILE="$directive_file" command lfops "$@" || exit_code=$?

        if [[ -s "$directive_file" ]]; then
            source "$directive_file"
            if [[ $exit_code -eq 0 ]]; then
                exit_code=$?
            fi
        fi

        rm -f "$directive_file"
        return "$exit_code"
    }
fi
"""

SHELL_INIT_BASH = """\
# loopflow shell integration for bash
#
# Enables directory switching after `lfops wt create`.

if command -v lfops >/dev/null 2>&1; then
    lfops() {
        local directive_file exit_code=0
        directive_file="$(mktemp)"

        LOOPFLOW_DIRECTIVE_FILE="$directive_file" command lfops "$@" || exit_code=$?

        if [[ -s "$directive_file" ]]; then
            source "$directive_file"
            if [[ $exit_code -eq 0 ]]; then
                exit_code=$?
            fi
        fi

        rm -f "$directive_file"
        return "$exit_code"
    }
fi
"""

SHELL_INIT_FISH = """\
# loopflow shell integration for fish
#
# Enables directory switching after `lfops wt create`.

if command -v lfops >/dev/null 2>&1
    function lfops
        set -l directive_file (mktemp)
        set -l exit_code 0

        LOOPFLOW_DIRECTIVE_FILE=$directive_file command lfops $argv; or set exit_code $status

        if test -s $directive_file
            source $directive_file
            if test $exit_code -eq 0
                set exit_code $status
            end
        end

        rm -f $directive_file
        return $exit_code
    end
end
"""

SHELLS = {
    "zsh": SHELL_INIT_ZSH,
    "bash": SHELL_INIT_BASH,
    "fish": SHELL_INIT_FISH,
}

SHELL_CONFIGS = {
    "zsh": Path.home() / ".zshrc",
    "bash": Path.home() / ".bashrc",
    "fish": Path.home() / ".config" / "fish" / "config.fish",
}

SHELL_INSTALL_LINE = {
    "zsh": 'if command -v lfops >/dev/null 2>&1; then eval "$(command lfops shell init zsh)"; fi',
    "bash": 'if command -v lfops >/dev/null 2>&1; then eval "$(command lfops shell init bash)"; fi',
    "fish": "if command -v lfops >/dev/null 2>&1; lfops shell init fish | source; end",
}


def write_directive(command: str) -> bool:
    """Write a shell command to the directive file if set. Returns True if written."""
    directive_file = os.environ.get("LOOPFLOW_DIRECTIVE_FILE")
    if directive_file:
        with open(directive_file, "a") as f:
            f.write(command + "\n")
        return True
    return False


def register_commands(app: typer.Typer) -> None:
    shell_app = typer.Typer(help="Shell integration setup")

    @shell_app.command("init")
    def shell_init(
        shell: Annotated[
            str,
            typer.Argument(help="Shell to generate code for (zsh, bash, fish)"),
        ] = "zsh",
    ) -> None:
        """Print shell integration code."""
        if shell not in SHELLS:
            typer.echo(f"Unknown shell: {shell}. Supported: {', '.join(SHELLS.keys())}", err=True)
            raise typer.Exit(1)

        typer.echo(SHELLS[shell])

    @shell_app.command("install")
    def shell_install(
        shell: Annotated[
            str | None,
            typer.Argument(help="Shell to install for (default: detect from $SHELL)"),
        ] = None,
        yes: Annotated[bool, typer.Option("--yes", "-y", help="Skip confirmation")] = False,
    ) -> None:
        """Install shell integration to config file."""
        if shell is None:
            current_shell = os.environ.get("SHELL", "")
            if "zsh" in current_shell:
                shell = "zsh"
            elif "bash" in current_shell:
                shell = "bash"
            elif "fish" in current_shell:
                shell = "fish"
            else:
                typer.echo("Could not detect shell. Specify: lfops shell install zsh", err=True)
                raise typer.Exit(1)

        if shell not in SHELLS:
            typer.echo(f"Unknown shell: {shell}. Supported: {', '.join(SHELLS.keys())}", err=True)
            raise typer.Exit(1)

        config_path = SHELL_CONFIGS[shell]
        install_line = SHELL_INSTALL_LINE[shell]

        # Check if already installed
        if config_path.exists():
            content = config_path.read_text()
            if "lfops shell init" in content:
                typer.echo(f"Already installed in {config_path}")
                return

        # Confirm
        if not yes:
            typer.echo(f"Will append to {config_path}:")
            typer.echo("")
            typer.echo(install_line)
            if not typer.confirm("Proceed?"):
                typer.echo("Aborted")
                raise typer.Exit(0)

        # Install
        config_path.parent.mkdir(parents=True, exist_ok=True)
        with open(config_path, "a") as f:
            f.write("\n" + install_line + "\n")

        typer.echo(f"Installed to {config_path}")
        typer.echo(f"Restart your shell or run: source {config_path}")

    app.add_typer(shell_app, name="shell")
