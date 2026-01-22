"""lfops: Loopflow operations CLI."""

import sys

import typer

from loopflow.lfops import abandon as abandon_module
from loopflow.lfops import add as add_module
from loopflow.lfops import commit as commit_module
from loopflow.lfops import cp as cp_module
from loopflow.lfops import init as init_module
from loopflow.lfops import land as land_module
from loopflow.lfops import pr as pr_module
from loopflow.lfops import rebase as rebase_module
from loopflow.lfops import shell as shell_module
from loopflow.lfops import summarize as summarize_module
from loopflow.lfops import sync as sync_module
from loopflow.lfops import wt as wt_module

app = typer.Typer(help="Loopflow operations")

# Register commands from submodules
abandon_module.register_commands(app)
add_module.register_commands(app)
cp_module.register_commands(app)
init_module.register_commands(app)
pr_module.register_commands(app)
land_module.register_commands(app)
commit_module.register_commands(app)
rebase_module.register_commands(app)
shell_module.register_commands(app)
summarize_module.register_commands(app)
sync_module.register_commands(app)
wt_module.register_commands(app)


def main() -> None:
    """Entry point for lfops command."""
    if len(sys.argv) == 1:
        sys.argv.append("doctor")
    app()


if __name__ == "__main__":
    main()
