from __future__ import annotations

import json
from typing import Optional

import typer
from rich.console import Console
from rich.table import Table

from loopflow import api
from loopflow.errors import LoopflowError
from loopflow.models import Wave


app = typer.Typer(help="Query lfd and manage waves.")
console = Console()


def _wave_table(waves: list[Wave]) -> Table:
    table = Table(show_header=True, header_style="bold")
    table.add_column("name")
    table.add_column("status")
    table.add_column("flow")
    table.add_column("iter", justify="right")
    table.add_column("repo")
    table.add_column("local_worktree")
    table.add_column("remote_branch")
    for wave in waves:
        run = wave.active_run
        local_worktree = (run.local_worktree if run else None) or wave.local_worktree or "-"
        remote_branch = (run.remote_branch if run else None) or wave.remote_branch or wave.branch or "-"
        table.add_row(
            wave.name,
            wave.status,
            wave.flow,
            str(wave.iteration),
            wave.repo,
            local_worktree,
            remote_branch,
        )
    return table


@app.callback(invoke_without_command=True)
def main(ctx: typer.Context, json_output: bool = typer.Option(False, "--json", "-j")) -> None:
    if ctx.invoked_subcommand is not None:
        return

    status = api.status()
    waves = api.waves()
    if json_output:
        typer.echo(json.dumps({"status": status, "waves": [w.model_dump(mode="json") for w in waves]}, indent=2))
        return

    console.print(
        f"lfd pid={status.get('pid', 'unknown')} "
        f"waves={status.get('waves_defined', 0)} "
        f"running={status.get('waves_running', 0)}"
    )
    if waves:
        console.print(_wave_table(waves))
    else:
        console.print("no waves")


@app.command("list")
def list_waves(
    repo: Optional[str] = None,
    json_output: bool = typer.Option(False, "--json", "-j"),
) -> None:
    waves = api.waves(repo=repo)
    if json_output:
        typer.echo(json.dumps([wave.model_dump(mode="json") for wave in waves], indent=2))
        return
    if waves:
        console.print(_wave_table(waves))
    else:
        console.print("no waves")


@app.command("show")
def show_wave(name_or_id: str, json_output: bool = typer.Option(False, "--json", "-j")) -> None:
    wave = api.wave(name_or_id)
    if wave is None:
        typer.echo(f"wave not found: {name_or_id}", err=True)
        raise typer.Exit(code=1)
    if json_output:
        typer.echo(json.dumps(wave.model_dump(mode="json"), indent=2))
        return

    data = wave.model_dump()
    table = Table(show_header=False)
    for key in ("id", "name", "status", "flow", "repo", "iteration"):
        if key in data:
            table.add_row(key, str(data[key]))
    console.print(table)
    if wave.active_run:
        active = Table(title="active_run", show_header=False)
        active.add_row("id", wave.active_run.id)
        active.add_row("status", wave.active_run.status)
        active.add_row("iteration", str(wave.active_run.iteration))
        console.print(active)


@app.command("create")
def create_wave(
    name: str,
    repo: str,
    flow: Optional[str] = None,
    direction: Optional[list[str]] = typer.Option(None, "--direction", "-d"),
    area: Optional[list[str]] = typer.Option(None, "--area", "-a"),
) -> None:
    wave = api.create_wave(name, repo, flow=flow, direction=direction, area=area)
    typer.echo(json.dumps(wave.model_dump(mode="json"), indent=2))


@app.command("run")
def run_wave(name_or_id: str) -> None:
    api.run_wave(name_or_id)


@app.command("stop")
def stop_wave(name_or_id: str) -> None:
    api.stop_wave(name_or_id)


@app.command("delete")
def delete_wave(name_or_id: str) -> None:
    api.delete_wave(name_or_id)


@app.command("land")
def land_wave(name_or_id: str) -> None:
    api.land_wave(name_or_id)


@app.command("logs")
def logs_wave(name_or_id: str) -> None:
    try:
        for line in api.wave_logs(name_or_id):
            typer.echo(line)
    except LoopflowError as exc:
        typer.echo(str(exc), err=True)
        raise typer.Exit(code=1) from exc
