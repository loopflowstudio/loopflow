from __future__ import annotations

import json
import time
import webbrowser
from typing import Optional

import typer
from rich.console import Console
from rich.table import Table

from loopflow import api
from loopflow.errors import LoopflowError
from loopflow.models import AuthProviderStatus, Repo, Wave


app = typer.Typer(help="Query lfd and manage waves.")
auth_app = typer.Typer(help="Manage provider authentication.")
repos_app = typer.Typer(help="Manage registered repositories.")
app.add_typer(auth_app, name="auth")
app.add_typer(repos_app, name="repos")
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


def _provider_label(provider: str) -> str:
    labels = {
        "github": "GitHub",
        "claude": "Claude",
        "codex": "Codex",
        "opencodezen": "OpenCode Zen",
    }
    return labels.get(provider.lower(), provider)


def _repo_table(repos: list[Repo]) -> Table:
    table = Table(show_header=True, header_style="bold")
    table.add_column("name")
    table.add_column("repo_id")
    table.add_column("path")
    table.add_column("waves", justify="right")
    table.add_column("registered")
    table.add_column("added_at")
    for repo in repos:
        table.add_row(
            repo.name,
            repo.repo_id,
            repo.path,
            str(repo.wave_count),
            "yes" if repo.registered else "no",
            repo.added_at.isoformat() if repo.added_at else "-",
        )
    return table


def _split_repo_slug(repo_id: str) -> tuple[str, str]:
    owner, sep, repo = repo_id.partition("/")
    if not sep or not owner or not repo or "/" in repo:
        raise typer.BadParameter("repo must be in owner/repo format")
    return owner, repo


def _auth_status_table(statuses: list[AuthProviderStatus]) -> Table:
    table = Table(show_header=True, header_style="bold")
    table.add_column("provider")
    table.add_column("status")
    table.add_column("details")

    for status in statuses:
        if status.status == "active":
            icon = "✓"
            if status.provider == "github" and status.login:
                details = f"@{status.login}"
            else:
                details = "authenticated"
        elif status.status == "pending":
            icon = "…"
            details = "waiting for browser confirmation"
        elif status.status == "expired":
            icon = "!"
            details = "expired"
        else:
            icon = "✗"
            details = "not connected"

        table.add_row(_provider_label(status.provider), f"{icon} {status.status}", details)

    return table


def _connect_provider(provider: str) -> None:
    flow = api.start_auth(provider)
    verification_url = flow.verification_uri_complete or flow.verification_uri
    provider_label = _provider_label(provider)

    typer.echo(f"Opening {provider_label} auth in your browser...")
    opened = webbrowser.open(verification_url)
    if not opened:
        typer.echo(verification_url)

    deadline = time.time() + 180
    while time.time() < deadline:
        status = api.auth_status(provider)
        if status.status == "active":
            if provider == "github" and status.login:
                typer.echo(f"✓ Authenticated as @{status.login}")
            else:
                typer.echo("✓ Authenticated")
            return
        if status.status == "expired":
            typer.echo("Authentication expired. Try again.")
            return
        time.sleep(1)

    typer.echo("Authentication still pending. Complete auth in browser and run `lfq auth status`.")


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


@app.command("list", help="List all waves.")
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


@app.command("show", help="Show details for a wave.")
def show_wave(name_or_id: str, json_output: bool = typer.Option(False, "--json", "-j")) -> None:
    wave = api.wave(name_or_id)
    if wave is None:
        typer.echo(f"wave not found: {name_or_id}. Run `lfq list` to see available waves.", err=True)
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


@app.command("create", help="Create a new wave.")
def create_wave(
    name: str,
    repo: str,
    flow: Optional[str] = None,
    direction: Optional[list[str]] = typer.Option(None, "--direction", "-d"),
    area: Optional[list[str]] = typer.Option(None, "--area", "-a"),
) -> None:
    wave = api.create_wave(name, repo, flow=flow, direction=direction, area=area)
    typer.echo(json.dumps(wave.model_dump(mode="json"), indent=2))


@app.command("run", help="Start a wave.")
def run_wave(name_or_id: str) -> None:
    api.run_wave(name_or_id)


@app.command("stop", help="Stop a running wave.")
def stop_wave(name_or_id: str) -> None:
    api.stop_wave(name_or_id)


@app.command("delete", help="Delete a wave.")
def delete_wave(name_or_id: str) -> None:
    api.delete_wave(name_or_id)


@app.command("land", help="Land a wave's PR via merge queue.")
def land_wave(name_or_id: str) -> None:
    api.land_wave(name_or_id)


@app.command("logs", help="Tail agent output for a wave.")
def logs_wave(name_or_id: str) -> None:
    try:
        for line in api.wave_logs(name_or_id):
            typer.echo(line)
    except LoopflowError as exc:
        typer.echo(str(exc), err=True)
        raise typer.Exit(code=1) from exc


@auth_app.command("status", help="Show authentication status for all providers.")
def auth_status(
    provider: Optional[str] = None,
    json_output: bool = typer.Option(False, "--json", "-j"),
) -> None:
    if provider is not None:
        status = api.auth_status(provider)
        if json_output:
            typer.echo(json.dumps(status.model_dump(mode="json"), indent=2))
            return
        console.print(_auth_status_table([status]))
        return

    statuses = api.auth_status()
    if json_output:
        typer.echo(json.dumps([status.model_dump(mode="json") for status in statuses], indent=2))
        return
    console.print(_auth_status_table(statuses))


@auth_app.command("github", help="Start GitHub authentication.")
def auth_github() -> None:
    _connect_provider("github")


@auth_app.command("claude", help="Start Claude authentication.")
def auth_claude() -> None:
    _connect_provider("claude")


@auth_app.command("codex", help="Start Codex authentication.")
def auth_codex() -> None:
    _connect_provider("codex")


@auth_app.command("disconnect", help="Disconnect a provider.")
def auth_disconnect(provider: str) -> None:
    status = api.disconnect_auth(provider)
    if status.status == "none":
        typer.echo(f"Disconnected {_provider_label(status.provider)}")
    else:
        typer.echo(f"Updated {_provider_label(status.provider)} status to {status.status}")


@repos_app.callback(invoke_without_command=True)
def repos_main(
    ctx: typer.Context,
    json_output: bool = typer.Option(False, "--json", "-j"),
) -> None:
    if ctx.invoked_subcommand is not None:
        return
    repos = api.list_repos()
    if json_output:
        typer.echo(json.dumps([repo.model_dump(mode="json") for repo in repos], indent=2))
        return
    if repos:
        console.print(_repo_table(repos))
    else:
        console.print("no repos")


@repos_app.command("add", help="Register an existing git repository path.")
def repos_add(path: str) -> None:
    repo = api.add_repo(path)
    typer.echo(json.dumps(repo.model_dump(mode="json"), indent=2))


@repos_app.command("rm", help="Unregister a repository path.")
def repos_rm(path: str) -> None:
    api.remove_repo(path)


@repos_app.command("children", help="List child repos for owner/repo.")
def repos_children(repo: str, json_output: bool = typer.Option(False, "--json", "-j")) -> None:
    owner, repo_name = _split_repo_slug(repo)
    repos = api.list_children(owner, repo_name)
    if json_output:
        typer.echo(json.dumps([entry.model_dump(mode="json") for entry in repos], indent=2))
        return
    if repos:
        console.print(_repo_table(repos))
    else:
        console.print("no child repos")


@repos_app.command("parents", help="List parent repos for owner/repo.")
def repos_parents(repo: str, json_output: bool = typer.Option(False, "--json", "-j")) -> None:
    owner, repo_name = _split_repo_slug(repo)
    repos = api.list_parents(owner, repo_name)
    if json_output:
        typer.echo(json.dumps([entry.model_dump(mode="json") for entry in repos], indent=2))
        return
    if repos:
        console.print(_repo_table(repos))
    else:
        console.print("no parent repos")


@repos_app.command("add-child", help="Add parent->child repo relationship.")
def repos_add_child(parent: str, child: str) -> None:
    parent_owner, parent_repo = _split_repo_slug(parent)
    child_owner, child_repo = _split_repo_slug(child)
    api.add_child(parent_owner, parent_repo, child_owner, child_repo)


@repos_app.command("rm-child", help="Remove parent->child repo relationship.")
def repos_rm_child(parent: str, child: str) -> None:
    parent_owner, parent_repo = _split_repo_slug(parent)
    child_owner, child_repo = _split_repo_slug(child)
    api.remove_child(parent_owner, parent_repo, child_owner, child_repo)
