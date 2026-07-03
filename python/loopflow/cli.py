from __future__ import annotations

import json
import os
import shutil
import time
import webbrowser
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional
from urllib.parse import parse_qs, urlparse

import typer
from rich.console import Console
from rich.table import Table

from loopflow import api
from loopflow.errors import LoopflowError
from loopflow.models import (
    AuthProviderStatus,
    ProviderInfo,
    Repo,
    Session,
    Wave,
)

app = typer.Typer(help="Query lfd and manage waves.")
auth_app = typer.Typer(help="Manage provider authentication.")
repos_app = typer.Typer(help="Manage registered repositories.")
worker_app = typer.Typer(help="Launch and inspect worker agents.")
wave_app = typer.Typer(help="Launch and inspect Wave agents.")
app.add_typer(auth_app, name="auth")
app.add_typer(repos_app, name="repos")
app.add_typer(worker_app, name="worker")
app.add_typer(wave_app, name="wave")
console = Console()


_PROVIDER_LABELS = {
    "github": "GitHub",
    "claude": "Claude",
    "codex": "Codex",
    "opencodezen": "OpenCode Zen",
    "asana": "Asana",
}

_PROVIDER_API_KEY_CONFIG = {
    "claude": ("ANTHROPIC_API_KEY", "Claude API key"),
    "codex": ("OPENAI_API_KEY", "Codex API key"),
    "opencodezen": ("OPENCODE_API_KEY", "OpenCode API key"),
}

_METERED_API_KEY_PROVIDERS = {"claude", "codex", "opencodezen"}

_PM_OAUTH_CONFIGURE_ERRORS = {
    "asana": "Asana requires OAuth. Run 'lf op auth asana' to connect.",
}


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
        remote_branch = (
            (run.remote_branch if run else None) or wave.remote_branch or wave.branch or "-"
        )
        table.add_row(
            wave.name,
            wave.status,
            wave.primary_flow,
            str(wave.iteration),
            wave.repo,
            local_worktree,
            remote_branch,
        )
    return table


def _format_wave_value(value: object) -> str:
    if isinstance(value, list):
        return ", ".join(str(item) for item in value) if value else "-"
    return str(value)


def _wave_detail_table(wave: Wave) -> Table:
    table = Table(show_header=False)
    rows = [
        ("id", wave.id),
        ("name", wave.name),
        ("status", wave.status),
        ("flow", wave.primary_flow),
        ("repo", wave.repo),
        ("iteration", wave.iteration),
        ("direction", wave.direction),
        ("area", wave.area),
    ]
    for key, value in rows:
        table.add_row(key, _format_wave_value(value))
    return table


def _short_id(value: object) -> str:
    text = str(value or "")
    if len(text) <= 12:
        return text or "-"
    return text[:12]


def _session_role(session: Session) -> str:
    return session.session_use or "-"


def _find_explicit_env_agent_session() -> Optional[Session]:
    session_id = os.environ.get("LFD_SESSION_ID")
    if session_id:
        try:
            return api.get_session(session_id)
        except LoopflowError:
            return None

    wave_id = os.environ.get("LFD_WAVE_ID")
    run_id = os.environ.get("LFD_RUN_ID")
    role = os.environ.get("LFD_AGENT_ROLE")
    if not any((wave_id, run_id, role)):
        return None

    matches = api.list_sessions()
    if wave_id:
        matches = [session for session in matches if session.wave_id == wave_id]
    if run_id:
        matches = [session for session in matches if session.run_id == run_id]
    if role:
        matches = [session for session in matches if _session_role(session) == role]
    if len(matches) == 1:
        return matches[0]
    return None


def _find_current_agent_session() -> Optional[Session]:
    explicit = _find_explicit_env_agent_session()
    if explicit is not None:
        return explicit

    if any(os.environ.get(name) for name in ("LFD_WAVE_ID", "LFD_RUN_ID", "LFD_AGENT_ROLE")):
        return None

    try:
        session = api.current_session(str(Path.cwd()))
    except LoopflowError:
        return None
    return session


def _session_id_from_attention(item: dict[str, Any]) -> Optional[str]:
    value = item.get("session_id")
    if isinstance(value, str):
        return value

    context = item.get("context")
    if not isinstance(context, dict):
        return None
    context_value = context.get("session_id")
    if isinstance(context_value, str):
        return context_value
    return None


def _needs_input_session_ids(items: list[dict[str, Any]]) -> set[str]:
    session_ids: set[str] = set()
    for item in items:
        if item.get("kind") != "interactive" or item.get("status") == "resolved":
            continue
        session_id = _session_id_from_attention(item)
        if session_id:
            session_ids.add(session_id)
    return session_ids


def _sessions_table(
    sessions: list[Session],
    attention_items: list[dict[str, Any]],
    wave_names: dict[str, str],
) -> Table:
    needs_input_ids = _needs_input_session_ids(attention_items)
    table = Table(show_header=True, header_style="bold")
    table.add_column("wave")
    table.add_column("session")
    table.add_column("role")
    table.add_column("step")
    table.add_column("status")
    table.add_column("needs-input")

    for session in sessions:
        session_id = session.id
        wave_id = session.wave_id
        table.add_row(
            wave_names.get(wave_id, _short_id(wave_id)),
            _short_id(session_id),
            _session_role(session),
            session.step or "-",
            session.status or "-",
            "yes" if session_id in needs_input_ids else "-",
        )
    return table


def _provider_label(provider: str) -> str:
    return _PROVIDER_LABELS.get(provider.lower(), provider)


def _provider_api_key_config(provider: str) -> Optional[tuple[str, str]]:
    return _PROVIDER_API_KEY_CONFIG.get(provider.lower())


def _pm_oauth_configure_error(provider: str) -> Optional[str]:
    return _PM_OAUTH_CONFIGURE_ERRORS.get(provider.lower())


def _provider_api_key_bills_per_token(provider: str) -> bool:
    return provider.lower() in _METERED_API_KEY_PROVIDERS


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


def _seconds_from_now(dt: datetime, now: datetime) -> float:
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return (dt - now).total_seconds()


def _format_relative_delta(seconds: float) -> str:
    total_seconds = max(0, int(seconds))
    if total_seconds < 60:
        return f"{total_seconds}s"
    if total_seconds < 3600:
        return f"{total_seconds // 60}m"
    if total_seconds < 86400:
        return f"{total_seconds // 3600}h"
    return f"{total_seconds // 86400}d"


def _status_details(status: AuthProviderStatus) -> str:
    if status.provider == "github" and status.login:
        details = f"@{status.login}"
    elif status.login:
        details = status.login
    else:
        details = "authenticated"

    now = datetime.now(timezone.utc)
    if status.expires_at is not None:
        delta = _seconds_from_now(status.expires_at, now)
        if delta <= 0:
            details = f"{details} · expired"
        else:
            details = f"{details} · expires {_format_relative_delta(delta)}"

    if status.next_refresh_at is not None:
        refresh_delta = _seconds_from_now(status.next_refresh_at, now)
        if refresh_delta <= 0:
            details = f"{details} · refreshing soon"
        else:
            details = f"{details} · refresh in {_format_relative_delta(refresh_delta)}"

    return details


def _auth_status_table(statuses: list[AuthProviderStatus]) -> Table:
    table = Table(show_header=True, header_style="bold")
    table.add_column("provider")
    table.add_column("status")
    table.add_column("details")

    for status in statuses:
        ct = status.credential_type or "oauth"
        if status.status == "active":
            details = _status_details(status)
            if ct == "apikey":
                icon = "⚠"
                status_label = "apikey"
                if _provider_api_key_bills_per_token(status.provider):
                    details = f"{details} · pay-per-token"
            else:
                icon = "✓"
                status_label = "oauth"
        elif status.status == "pending":
            icon = "…"
            status_label = "pending"
            details = "waiting for browser confirmation"
        elif status.status == "expired":
            icon = "!"
            status_label = "expired"
            details = "expired"
        else:
            icon = "✗"
            status_label = "none"
            details = "not connected"

        table.add_row(_provider_label(status.provider), f"{icon} {status_label}", details)

    return table


def _auth_poll_timeout_seconds(expires_in: Optional[int]) -> int:
    if expires_in is not None and expires_in > 0:
        return expires_in
    return 180


def _connect_provider(provider: str) -> None:
    flow = api.start_auth(provider)
    verification_url = flow.verification_uri_complete or flow.verification_uri
    provider_label = _provider_label(provider)

    typer.echo(f"Opening {provider_label} auth in your browser...")
    opened = webbrowser.open(verification_url)
    if not opened:
        typer.echo(verification_url)

    if provider == "asana":
        _complete_oauth_code_flow(provider)

    deadline = time.time() + _auth_poll_timeout_seconds(flow.expires_in)
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


def _complete_oauth_code_flow(provider: str) -> None:
    provider_label = _provider_label(provider)
    typer.echo(
        f"{provider_label} will redirect to an out-of-band page. "
        "Paste the full redirect URL or just the authorization code."
    )
    value = typer.prompt("Authorization code").strip()
    code = _extract_authorization_code(value)
    if not code:
        typer.echo("Error: could not find an authorization code in the pasted value", err=True)
        raise typer.Exit(1)
    api.complete_auth(provider, code)


def _extract_authorization_code(value: str) -> str:
    parsed = urlparse(value)
    if parsed.scheme and (parsed.query or parsed.fragment):
        query = parse_qs(parsed.query)
        if "code" in query and query["code"]:
            return query["code"][0]
        fragment = parse_qs(parsed.fragment)
        if "code" in fragment and fragment["code"]:
            return fragment["code"][0]
    return value.strip()


def _configure_provider_token(provider: str, env_name: str, prompt_label: str) -> None:
    api_key = os.environ.get(env_name)
    if not api_key:
        api_key = typer.prompt(prompt_label, hide_input=True).strip()
    if not api_key:
        typer.echo(f"Error: {env_name} is empty", err=True)
        raise typer.Exit(1)

    if _provider_api_key_bills_per_token(provider):
        typer.echo("API key auth bills per token. OAuth uses your existing subscription.")
    status = api.configure_api_key(provider, api_key)
    if _provider_api_key_bills_per_token(status.provider):
        typer.echo(f"{_provider_label(status.provider)} switched to API key")
    else:
        typer.echo(f"Stored {_provider_label(status.provider)} API key")


def _providers_table(providers: list[ProviderInfo]) -> Table:
    table = Table(show_header=True, header_style="bold")
    table.add_column("provider")
    table.add_column("status")
    table.add_column("billing")
    table.add_column("models")

    for p in providers:
        if p.auth_status == "active":
            status = "\u2713 active"
        else:
            status = f"\u2717 {p.auth_status}"
        model_names = ", ".join(m.display_name for m in p.models)
        table.add_row(_provider_label(p.provider), status, p.billing, model_names)
    return table


@app.callback(invoke_without_command=True)
def main(ctx: typer.Context, json_output: bool = typer.Option(False, "--json", "-j")) -> None:
    if ctx.invoked_subcommand is not None:
        return

    status = api.status()
    waves = api.waves()
    if json_output:
        data = {"status": status, "waves": [w.model_dump(mode="json") for w in waves]}
        typer.echo(json.dumps(data, indent=2))
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
        msg = f"wave not found: {name_or_id}. Run `lfq list` to see available waves."
        typer.echo(msg, err=True)
        raise typer.Exit(code=1)
    if json_output:
        typer.echo(json.dumps(wave.model_dump(mode="json"), indent=2))
        return

    console.print(_wave_detail_table(wave))
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


def _print_launch_response(response: Session, label: str) -> None:
    typer.echo(f"{label} {response.id}")
    typer.echo(f"wave    {response.wave_id}")
    typer.echo(f"use     {response.session_use}")
    typer.echo(f"run     {response.run_id or '-'}")
    typer.echo(f"attach  lfq attach {response.id}")


def _print_json_or_launch_response(response: Session, label: str, json_output: bool) -> None:
    if json_output:
        typer.echo(json.dumps(response.model_dump(mode="json", by_alias=True), indent=2))
        return
    _print_launch_response(response, label)


def _run_wave_command(name_or_id: str, json_output: bool) -> None:
    _print_json_or_launch_response(api.run_wave(name_or_id), "run", json_output)


@app.command("run", help="Ride a wave.")
def run_wave(name_or_id: str, json_output: bool = typer.Option(False, "--json", "-j")) -> None:
    _run_wave_command(name_or_id, json_output)


@wave_app.command("run", help="Start or attach the canonical Wave-agent session.")
def run(name_or_id: str, json_output: bool = typer.Option(False, "--json", "-j")) -> None:
    _run_wave_command(name_or_id, json_output)


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


@app.command("sessions", help="List live terminal sessions.")
def sessions(wave: Optional[str] = typer.Argument(None)) -> None:
    wave_id = None
    wave_names: dict[str, str] = {}
    if wave is None:
        wave_names = {item.id: item.name for item in api.waves()}
    else:
        resolved_wave = api.wave(wave)
        if resolved_wave is None:
            typer.echo(f"wave not found: {wave}. Run `lfq list` to see available waves.", err=True)
            raise typer.Exit(code=1)
        wave_id = resolved_wave.id
        wave_names[resolved_wave.id] = resolved_wave.name

    sessions_payload = api.list_sessions(wave_id=wave_id)
    attention_items = api.list_attention(status="unresolved")
    if sessions_payload:
        console.print(_sessions_table(sessions_payload, attention_items, wave_names))
    else:
        console.print("no sessions")


@app.command("whoami", help="Show the current lfd agent identity.")
def whoami(json_output: bool = typer.Option(False, "--json", "-j")) -> None:
    session = _find_current_agent_session()
    if session is None:
        typer.echo(
            "current session not found; run inside an lfd-managed agent session "
            "or set LFD_SESSION_ID",
            err=True,
        )
        raise typer.Exit(code=1)

    if json_output:
        typer.echo(json.dumps(session.model_dump(mode="json", by_alias=True), indent=2))
        return

    table = Table(show_header=False)
    table.add_row("session", session.id)
    table.add_row("wave", session.wave_id)
    table.add_row("run", session.run_id or "-")
    table.add_row("role", _session_role(session))
    table.add_row("cwd", session.cwd)
    table.add_row("status", session.status)
    console.print(table)


@worker_app.command("run", help="Launch a PR-producing worker agent.")
def worker_run(
    wave: Optional[str] = typer.Argument(None),
    flow: str = typer.Option(..., "--flow", "-f"),
    task: str = typer.Option(..., "--task", "-t"),
    json_output: bool = typer.Option(False, "--json", "-j"),
) -> None:
    has_env_identity = any(
        os.environ.get(name)
        for name in ("LFD_SESSION_ID", "LFD_WAVE_ID", "LFD_RUN_ID", "LFD_AGENT_ROLE")
    )
    caller = _find_current_agent_session() if wave is None or has_env_identity else None
    target_wave = wave or os.environ.get("LFD_WAVE_ID") or (caller.wave_id if caller else None)
    if target_wave is None:
        typer.echo(
            "wave is required outside an lfd-managed wave session",
            err=True,
        )
        raise typer.Exit(code=1)

    try:
        response = api.run_worker(
            target_wave,
            flow,
            task,
            parent_session_id=caller.id if caller else None,
        )
    except LoopflowError as exc:
        typer.echo(str(exc), err=True)
        raise typer.Exit(code=1) from exc

    _print_json_or_launch_response(response, "worker_run", json_output)


@app.command("attach", help="Attach to a terminal session.")
def attach(session_id: str) -> None:
    try:
        connection = api.attach_session(session_id)
    except LoopflowError as exc:
        typer.echo(str(exc), err=True)
        raise typer.Exit(code=1) from exc

    if shutil.which("tmux") is None:
        typer.echo("Error: tmux is not available on PATH", err=True)
        raise typer.Exit(code=1)

    try:
        os.execvp("tmux", ["tmux", "attach", "-t", connection.session_name])
    except OSError as exc:
        typer.echo(
            f"Error: failed to attach tmux session {connection.session_name}: {exc}",
            err=True,
        )
        raise typer.Exit(code=1) from exc


@app.command("providers", help="List providers with auth status and models.")
def providers_cmd(
    json_output: bool = typer.Option(False, "--json", "-j"),
) -> None:
    provider_list = api.providers()
    if json_output:
        typer.echo(json.dumps([p.model_dump(mode="json") for p in provider_list], indent=2))
        return
    if provider_list:
        console.print(_providers_table(provider_list))
    else:
        console.print("no providers")


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


@auth_app.command("zen", help="Start OpenCode Zen authentication.")
def auth_zen() -> None:
    _connect_provider("opencodezen")


@auth_app.command("asana", help="Start Asana OAuth authentication.")
def auth_asana() -> None:
    _connect_provider("asana")


@auth_app.command("disconnect", help="Disconnect a provider.")
def auth_disconnect(provider: str) -> None:
    status = api.disconnect_auth(provider)
    if status.status == "none":
        typer.echo(f"Disconnected {_provider_label(status.provider)}")
    else:
        typer.echo(f"Updated {_provider_label(status.provider)} status to {status.status}")


@auth_app.command(
    "configure",
    help="Use an API key for a provider. To switch back to OAuth, run `lfq auth <provider>`.",
)
def auth_configure(
    provider: str,
) -> None:
    message = _pm_oauth_configure_error(provider)
    if message is not None:
        typer.echo(f"Error: {message}", err=True)
        raise typer.Exit(1)

    config = _provider_api_key_config(provider)
    if config is None:
        typer.echo(f"Error: {provider} does not support API key auth", err=True)
        raise typer.Exit(1)

    env_name, _ = config
    _configure_provider_token(provider, env_name, f"{_provider_label(provider)} API key")


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
