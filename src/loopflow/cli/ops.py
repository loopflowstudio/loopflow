"""Management and maintenance subcommands."""

import typer

from loopflow.cli import compare as compare_module
from loopflow.cli import land as land_module
from loopflow.cli import maestro, meta, pr, sessions, status

app = typer.Typer(name="ops", help="Management and maintenance commands.")

app.add_typer(pr.app, name="pr")
app.add_typer(meta.app, name="meta")
app.add_typer(maestro.app, name="maestro")

app.command()(status.status)
app.command()(sessions.stop)
app.command()(sessions.prune)
app.command()(compare_module.compare)
app.command()(land_module.land)
