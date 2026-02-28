from __future__ import annotations

import json
import time
import webbrowser
from datetime import datetime, timezone
from typing import Optional

import typer
from rich.console import Console
from rich.table import Table

from loopflow import api
from loopflow.errors import LoopflowError
from loopflow.models import (
    AuthProviderStatus,
    CostRates,
    ProviderInfo,
    Repo,
    TokenTotals,
    UsageSummary,
    UsageSummaryGroup,
    Wave,
)

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
            if ct == "apikey":
                icon = "⚠"
                status_label = "apikey"
            else:
                icon = "✓"
                status_label = "oauth"
            details = _status_details(status)
            if ct == "apikey":
                details = f"{details} · pay-per-token"
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


def _format_tokens(value: int) -> str:
    if value == 0:
        return "\u2014"
    if value < 1000:
        return str(value)
    if value < 1_000_000:
        return f"{value / 1000:.1f}k"
    return f"{value / 1_000_000:.1f}M"


_GROUP_BY_FOR_FILTER = {
    "wave": "step",
    "flow": "wave",
    "step": "wave",
    "model": "wave",
    "source": "wave",
}


def _infer_group_by(
    wave: Optional[str],
    flow: Optional[str],
    step: Optional[str],
    model: Optional[str],
    source: Optional[str],
    prompt: bool,
    group_by: Optional[str],
) -> str:
    if group_by is not None:
        return group_by
    if prompt:
        return "source"
    filters = {
        k: v
        for k, v in {
            "wave": wave,
            "flow": flow,
            "step": step,
            "model": model,
            "source": source,
        }.items()
        if v is not None
    }
    if len(filters) > 1:
        typer.echo(
            "Multiple filters require --group-by. "
            "Example: lfq usage --wave X --model Y --group-by step",
            err=True,
        )
        raise typer.Exit(code=1)
    if len(filters) == 1:
        filter_name = next(iter(filters))
        return _GROUP_BY_FOR_FILTER[filter_name]
    return "wave"


def _usage_table(summary: UsageSummary) -> Table:
    table = Table(show_header=True, header_style="bold")
    table.add_column(summary.group_by)
    table.add_column("input", justify="right")
    table.add_column("output", justify="right")
    table.add_column("reasoning", justify="right")
    table.add_column("cache_r", justify="right")
    table.add_column("cache_w", justify="right")
    table.add_column("sessions", justify="right")
    table.add_column("turns", justify="right")

    for group in summary.groups:
        t = group.tokens
        table.add_row(
            group.key,
            _format_tokens(t.input),
            _format_tokens(t.output),
            _format_tokens(t.reasoning),
            _format_tokens(t.cache_read),
            _format_tokens(t.cache_write),
            str(group.sessions),
            str(group.turns),
        )
    return table


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


def _estimate_cost(tokens: TokenTotals, rates: CostRates) -> float:
    cost = tokens.input * rates.input_per_mtok / 1_000_000
    cost += tokens.output * rates.output_per_mtok / 1_000_000
    if rates.cache_read_per_mtok:
        cost += tokens.cache_read * rates.cache_read_per_mtok / 1_000_000
    if rates.cache_write_per_mtok:
        cost += tokens.cache_write * rates.cache_write_per_mtok / 1_000_000
    return cost


def _format_cost(cost: float) -> str:
    if cost < 0.01:
        return "<$0.01"
    return f"~${cost:.2f}"


def _billing_tables(
    summary: UsageSummary, providers: list[ProviderInfo]
) -> tuple[list[Table], Optional[str]]:
    model_billing: dict[str, tuple[str, Optional[CostRates]]] = {}
    for p in providers:
        for m in p.models:
            model_billing[m.id] = (p.billing, m.cost_rates)

    sub_groups: list[UsageSummaryGroup] = []
    metered_groups: list[tuple[UsageSummaryGroup, Optional[CostRates]]] = []
    for group in summary.groups:
        billing, rates = model_billing.get(group.key, ("subscription", None))
        if billing == "per_token":
            metered_groups.append((group, rates))
        else:
            sub_groups.append(group)

    tables: list[Table] = []

    if sub_groups:
        table = Table(title="Subscription", show_header=True, header_style="bold")
        table.add_column("model")
        table.add_column("input", justify="right")
        table.add_column("output", justify="right")
        table.add_column("sessions", justify="right")
        table.add_column("turns", justify="right")
        for group in sub_groups:
            t = group.tokens
            table.add_row(
                group.key,
                _format_tokens(t.input),
                _format_tokens(t.output),
                str(group.sessions),
                str(group.turns),
            )
        tables.append(table)

    total_cost = 0.0
    if metered_groups:
        table = Table(title="Metered", show_header=True, header_style="bold")
        table.add_column("model")
        table.add_column("input", justify="right")
        table.add_column("output", justify="right")
        table.add_column("sessions", justify="right")
        table.add_column("turns", justify="right")
        table.add_column("est. cost", justify="right")
        for group, rates in metered_groups:
            t = group.tokens
            cost = _estimate_cost(t, rates) if rates else 0.0
            total_cost += cost
            table.add_row(
                group.key,
                _format_tokens(t.input),
                _format_tokens(t.output),
                str(group.sessions),
                str(group.turns),
                _format_cost(cost) if rates else "\u2014",
            )
        tables.append(table)

    total_line = f"total metered: {_format_cost(total_cost)}" if total_cost > 0 else None
    return tables, total_line


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


@app.command("run", help="Ride a wave.")
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


@app.command("usage", help="Show token usage summary.")
def usage(
    wave: Optional[str] = typer.Option(None, "--wave", "-w"),
    flow: Optional[str] = typer.Option(None, "--flow", "-f"),
    step: Optional[str] = typer.Option(None, "--step", "-s"),
    model: Optional[str] = typer.Option(None, "--model", "-m"),
    source: Optional[str] = typer.Option(None, "--source"),
    prompt: bool = typer.Option(False, "--prompt", "-p"),
    billing: bool = typer.Option(False, "--billing", "-b"),
    group_by: Optional[str] = typer.Option(None, "--group-by", "-g"),
    from_time: Optional[str] = typer.Option(None, "--from"),
    to_time: Optional[str] = typer.Option(None, "--to"),
    json_output: bool = typer.Option(False, "--json", "-j"),
) -> None:
    resolved_group_by = (
        "model" if billing else _infer_group_by(wave, flow, step, model, source, prompt, group_by)
    )
    summary = api.usage_summary(
        group_by=resolved_group_by,
        wave=wave,
        flow=flow,
        step=step,
        model=model,
        source=source,
        from_=from_time,
        to_=to_time,
    )
    if json_output:
        typer.echo(json.dumps(summary.model_dump(mode="json", by_alias=True), indent=2))
        return
    if billing:
        provider_list = api.providers()
        tables, total_line = _billing_tables(summary, provider_list)
        if not tables:
            console.print("no usage data")
            return
        for table in tables:
            console.print(table)
        if total_line:
            console.print(total_line)
        return
    if summary.groups:
        console.print(_usage_table(summary))
    else:
        console.print("no usage data")


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


@auth_app.command("disconnect", help="Disconnect a provider.")
def auth_disconnect(provider: str) -> None:
    status = api.disconnect_auth(provider)
    if status.status == "none":
        typer.echo(f"Disconnected {_provider_label(status.provider)}")
    else:
        typer.echo(f"Updated {_provider_label(status.provider)} status to {status.status}")


@auth_app.command("configure", help="Switch credential type for a provider.")
def auth_configure(
    provider: str,
    credential: str = typer.Option(..., "--credential", "-c", help="oauth or apikey"),
) -> None:
    if credential not in ("oauth", "apikey"):
        typer.echo("Error: --credential must be 'oauth' or 'apikey'", err=True)
        raise typer.Exit(1)

    api_key = None
    if credential == "apikey":
        env_names = {
            "claude": "ANTHROPIC_API_KEY",
            "codex": "OPENAI_API_KEY",
            "opencodezen": "OPENCODE_API_KEY",
        }
        env_name = env_names.get(provider.lower())
        if env_name:
            import os

            api_key = os.environ.get(env_name)
        if not api_key:
            typer.echo(
                f"Error: set {env_name or 'the API key env var'} in your environment first",
                err=True,
            )
            raise typer.Exit(1)
        typer.echo(f"⚠ API key auth bills per token. OAuth uses your existing subscription.")

    status = api.configure_credential(provider, credential, api_key=api_key)
    label = _provider_label(status.provider)
    ct = status.credential_type or credential
    typer.echo(f"{label} credential type set to {ct}")


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
