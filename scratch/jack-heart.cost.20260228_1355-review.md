# lfq usage & providers — design review

## What was implemented

Three additions to the `lfq` CLI, all client-only Python:

- **`lfq usage`** — queries `GET /v0/usage/summary` and renders a token summary table. Supports `--wave`, `--flow`, `--step`, `--model`, `--prompt`, `--group-by`, `--from`, `--to`, `--json`.
- **`lfq providers`** — queries `GET /v0/providers` and renders a table with auth status, billing type, and model list. Supports `--json`.
- **`lfq auth zen`** — connects OpenCode Zen via the existing `_connect_provider` pattern.

## Key choices

**Smart default grouping.** When a single filter is given (e.g. `--wave engbot`), the CLI infers the most useful `group_by` dimension (`step` for wave, `wave` for everything else). Multiple filters require explicit `--group-by` — errors clearly rather than guessing.

**Em dash for zeros.** Dense token tables use `—` for zero values so non-zero data pops. `--json` gives exact integers for scripting.

**`--prompt` as sugar.** `--prompt` sets `group_by=source`. More discoverable than `--group-by source` for the common "where are my input tokens coming from?" question.

**Reused `_providers_table` for `lfq providers`.** The `providers` command reuses the existing `api.providers()` endpoint. The table includes billing type and model list — data already present in `ProviderInfo`.

## How it fits together

```
CLI (cli.py)
├── usage command → _infer_group_by → api.usage_summary → client.usage_summary
│                                                          └→ GET /v0/usage/summary
├── providers command → api.providers → client.providers
│                                       └→ GET /v0/providers
└── auth zen command → _connect_provider("opencodezen")
```

Three new Pydantic models (`TokenTotals`, `UsageSummaryGroup`, `UsageSummary`) mirror the API response shape. `UsageSummary` uses field aliases (`from`/`to`) since these are Python reserved words.

## Risks and bottlenecks

- **No backend changes** — this is pure client-side. If the `/v0/usage/summary` response shape diverges, `UsageSummary` validation will fail fast with a clear Pydantic error.
- **Token formatting precision** — `_format_tokens` uses one decimal place for k/M. Values like 999,950 render as `1000.0k` rather than `1.0M`. Acceptable for a summary view; `--json` gives exact values.

## What's not included

- **`--billing` split** — backend doesn't expose `auth_type` in usage aggregation yet. Design doc notes this as a future addition.
- **Timeseries CLI output** — belongs in the analytics dashboard, not the terminal.
- **Dollar cost estimates** — `cost_usd` on TurnUsage is sparse. Ship token counts now, add costs when data is reliable.
- **`--source` filter flag** — `source` is exposed as a programmatic API parameter but not as a CLI flag. `--prompt` covers the primary use case (group by source).
