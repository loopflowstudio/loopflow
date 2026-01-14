"""Management and maintenance subcommands."""

import typer

from loopflow.cli import commit as commit_module
from loopflow.cli import compare as compare_module
from loopflow.cli import land as land_module
from loopflow.cli import meta, pr, sessions, status

app = typer.Typer(name="ops", help="Management and maintenance commands.")

app.add_typer(pr.app, name="pr")

app.command()(meta.init)
app.command()(meta.install)
app.command()(meta.doctor)
app.command()(meta.version)
app.command()(status.status)
app.command()(sessions.stop)
app.command()(sessions.prune)
app.command()(commit_module.commit)
app.command()(compare_module.compare)
app.command()(land_module.land)
