from __future__ import annotations

import json
from typing import Optional

import typer

import loopflow
from loopflow.errors import LoopflowError


app = typer.Typer(help="Query lfd and manage waves.")


@app.callback(invoke_without_command=True)
def main(ctx: typer.Context) -> None:
    if ctx.invoked_subcommand is not None:
        return

    status = loopflow.status()
    waves = loopflow.waves()
    typer.echo(json.dumps({"status": status, "waves": [w.model_dump() for w in waves]}, indent=2))


@app.command("list")
def list_waves(repo: Optional[str] = None) -> None:
    waves = loopflow.waves(repo=repo)
    typer.echo(json.dumps([wave.model_dump() for wave in waves], indent=2))


@app.command("show")
def show_wave(name_or_id: str) -> None:
    wave = loopflow.wave(name_or_id)
    if wave is None:
        raise typer.Exit(code=1)
    typer.echo(json.dumps(wave.model_dump(), indent=2))


@app.command("create")
def create_wave(
    name: str,
    repo: str,
    flow: Optional[str] = None,
    direction: Optional[list[str]] = typer.Option(None, "--direction", "-d"),
    area: Optional[list[str]] = typer.Option(None, "--area", "-a"),
) -> None:
    wave = loopflow.create_wave(name, repo, flow=flow, direction=direction, area=area)
    typer.echo(json.dumps(wave.model_dump(), indent=2))


@app.command("run")
def run_wave(name_or_id: str) -> None:
    loopflow.run_wave(name_or_id)


@app.command("stop")
def stop_wave(name_or_id: str) -> None:
    loopflow.stop_wave(name_or_id)


@app.command("delete")
def delete_wave(name_or_id: str) -> None:
    loopflow.delete_wave(name_or_id)

