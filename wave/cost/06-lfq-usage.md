# 06: lfq Usage & Providers

**Finish line:** `lfq usage --wave engbot` prints a token summary table. `lfq providers` lists providers with auth status and billing model. `lfq auth zen` connects OpenCode Zen.

## What to build

Two CLI commands backed by thin client methods hitting existing HTTP endpoints. No backend changes.

### `lfq usage`

Calls `GET /v0/usage/summary`. Renders a Rich table.

```
$ lfq usage --wave engbot
engbot usage
         input    output  reasoning  cache_read  cache_write  sessions  turns
implement  42.1k    18.3k      2.1k       31.0k        8.2k         3     47
gate        8.4k     3.1k      0.5k        6.2k        1.8k         2     12
compress    5.2k     2.0k      0.3k        4.0k        1.1k         1      8
─────────────────────────────────────────────────────────────────────────────
total      55.7k    23.4k      2.9k       41.2k       11.1k         6     67
```

Flags:

| Flag | API param | group_by |
|------|-----------|----------|
| `--wave NAME` | `wave=NAME` | `step` |
| `--model NAME` | `model=NAME` | `model` |
| `--step NAME` | `step=NAME` | `wave` |
| `--from DATE` | `from=DATE` | (default) |
| `--to DATE` | `to=DATE` | (default) |
| `--group-by X` | `group_by=X` | explicit |
| `--prompt` | `group_by=source` | `source` |
| `--json` / `-j` | — | raw JSON |

Default `group_by` when no flag implies one: `wave`.

`--prompt` is sugar for `--group-by source` — shows where input tokens come from (step, diff, wave docs, area, etc.).

### `lfq providers`

Calls `GET /v0/providers`. Client method already exists. Table with provider, auth status, billing model, models.

### `lfq auth zen`

New subcommand under `auth_app`. Same pattern as `auth_github` — calls `_connect_provider("opencodezen")`.

## Key decisions

- **No `--billing` in v0.** `auth_type` is not yet on TurnUsage. Ship `usage` and `providers` first; `--billing` is a fast follow once the backend surfaces `auth_type` per turn.
- **Client-side token formatting.** `42.1k` for display. Raw values via `--json`.
- **`--prompt` as sugar.** Prompt composition is just `--group-by source`. No separate command.

## Scope

In:
- `lfq usage` with all flags above
- `lfq providers` with `--json`
- `lfq auth zen`
- Python client `usage_summary()` method
- Pydantic models for usage response types
- Tests for client, CLI output, token formatting

Out:
- `--billing` (needs backend `auth_type` per turn)
- Timeseries / `--bucket` display
- Cost estimation in CLI output
- Backend/Rust changes

## Implementation

1. **Models** (`python/loopflow/models.py`): `UsageSummaryGroup`, `UsageSummary` pydantic models matching `UsageSummaryDto`.
2. **Client** (`python/loopflow/client.py`): `usage_summary()` calling `GET /v0/usage/summary`.
3. **API** (`python/loopflow/api.py`): `usage_summary()` thin wrapper.
4. **CLI** (`python/loopflow/cli.py`): `_format_tokens()`, `_usage_table()`, `usage` command, `providers` command, `auth zen` subcommand.
5. **Tests**: token formatting, CLI commands with mocked API, provider table rendering.

## Done when

```bash
lfq usage
lfq usage --wave engbot
lfq usage --prompt
lfq usage --json
lfq providers
lfq providers --json
lfq auth zen
uv run pytest python/tests/ -v -k usage
uv run pytest python/tests/ -v -k provider
```
