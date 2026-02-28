# lfq usage & providers

## Problem

Operators running 10+ parallel waves have no terminal visibility into token consumption. The backend APIs exist (`/v0/usage/summary`, `/v0/providers`), the Swift analytics dashboard exists, but there's no CLI. `lfq usage --wave engbot` should answer "how much is this wave burning?" in one command, composable with shell tools.

## Approach

Three additions to the existing lfq CLI, all client-only Python:

1. **`lfq usage`** — top-level command calling `GET /v0/usage/summary`. Rich table output with token columns. Flags select the grouping dimension and filters.
2. **`lfq providers`** — top-level command calling `GET /v0/providers`. Table of providers with auth status and model list.
3. **`lfq auth zen`** — subcommand under `auth_app`. Same pattern as `auth github`/`auth claude`/`auth codex`.

### `lfq usage`

```bash
lfq usage                         # group by wave (default)
lfq usage --wave engbot           # filter to one wave, group by step
lfq usage --flow build            # filter to one flow, group by wave
lfq usage --model opus            # filter to one model, group by wave
lfq usage --step implement        # filter to one step, group by wave
lfq usage --prompt                # group by source (prompt composition)
lfq usage --group-by model        # explicit group-by
lfq usage --from 2026-02-01       # time-filtered
lfq usage --from 2026-02-01 --to 2026-02-14
lfq usage --json                  # raw JSON for piping
lfq usage --wave engbot --model opus --group-by step  # multiple filters require --group-by
```

**Default behavior:** When no `--group-by` is explicit, infer from a single filter:
- `--wave X` → group by `step` (most useful: what steps burn tokens in this wave?)
- `--flow X` → group by `wave`
- `--step X` → group by `wave`
- `--model X` → group by `wave`
- No filter → group by `wave`
- `--prompt` → group by `source` (overrides)

Multiple filters are intersection (backend ANDs them). When more than one filter is given, `--group-by` is required — error with a clear message rather than picking a surprising default.

**Table output:**

```
               input    output   reasoning  cache_r  cache_w  sessions  turns
engbot         42.1k    8.5k     —          —        —        3         45
infra          128.3k   24.1k    12.0k      45.0k    3.2k     8         120
ux             15.2k    3.1k     —          —        —        2         18
```

Token values formatted as `42.1k`, `1.2M`, or raw integers below 1000. Zero values shown as `—` for scannability. `--json` returns the raw API response with exact integer values.

### `lfq providers`

```bash
lfq providers                     # table output
lfq providers --json              # raw JSON
```

```
provider       status        billing        models
Claude         ✓ active      subscription   opus, sonnet, haiku
Codex          ✗ none        subscription   codex
OpenCode       ✓ active      per_token      kimi-k2, qwen3-coder, qwen3-max
```

### `lfq auth zen`

```bash
lfq auth zen                      # opens OpenCode Zen auth in browser
```

Same pattern as the three existing auth commands. Calls `_connect_provider("opencodezen")`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Subcommand group (`lfq usage summary`, `lfq usage timeseries`) | More structured | Over-engineering. Flags on one command are simpler and composable. Timeseries belongs in the dashboard, not terminal |
| Include `--billing` to split subscription vs metered | Richer output | Backend doesn't expose `auth_type` in usage aggregation yet. No "harness" group_by. Ship without, add when backend supports it |
| Client-side cost computation | Show dollar estimates | `CostRates` exist for some models but cost_usd on TurnUsage is sparse. The summary API returns token counts, not costs. Premature — add when the data is reliable |
| Separate `lfq tokens` command | Avoids overloading "usage" | "Usage" is the API name, the wave name, and what operators think of. Consistency wins |

## Key decisions

**Smart default grouping.** `lfq usage --wave engbot` groups by step because that's the useful follow-up question. The API requires `group_by` — the CLI infers it from context rather than making the user spell it out every time.

**Dashes for zeros.** Token tables are dense. Zero values as `—` make non-zero values pop. The `--json` escape hatch gives exact numbers when needed.

**No timeseries in CLI.** The analytics dashboard handles time-bucketed visualization. The CLI provides point-in-time snapshots. One tool, one job.

**`--prompt` as sugar.** `--prompt` sets `group_by=source`. It's more discoverable than `--group-by source` for the common "where are my input tokens coming from?" question.

**No `--billing` in v0.** The backend groups by wave/flow/step/model/source. There's no harness or auth_type dimension. Ship the useful thing now, add billing views when the backend supports them.

## Scope

In scope:
- `lfq usage` command with `--wave`, `--flow`, `--model`, `--step`, `--prompt`, `--group-by`, `--from`, `--to`, `--json`
- `lfq providers` command with `--json` (includes billing column)
- `lfq auth zen` subcommand
- Pydantic models for usage API response (`TokenTotals`, `UsageSummaryGroup`, `UsageSummary`)
- Client method `usage_summary()` → `GET /v0/usage/summary`
- Token formatting helper (`_format_tokens`)
- Tests for token formatting and CLI commands (mocked API)

Out of scope:
- Backend/Rust changes
- `--billing` split (needs backend `auth_type` in usage aggregation)
- Timeseries CLI output
- Dollar cost estimates in table output

## Implementation layers

1. **Models** (`models.py`): `TokenTotals`, `UsageSummaryGroup`, `UsageSummary`
2. **Client** (`client.py`): `usage_summary(group_by, wave, flow, step, model, from_, to_)` → `GET /v0/usage/summary`
3. **API** (`api.py`): thin wrapper `usage_summary()`
4. **CLI** (`cli.py`): `_format_tokens()`, `_usage_table()`, `_providers_table()`, `usage` command, `providers` command, `auth zen` command
5. **Tests**: token formatting, CLI with mocked API, provider table rendering

## Done when

```bash
lfq usage --wave engbot          # prints token summary table to terminal
lfq usage --flow build           # prints token summary for a flow
lfq usage --prompt               # prints prompt composition by source
lfq providers                    # lists providers with models, billing, and auth status
lfq auth zen                     # opens OpenCode Zen auth flow in browser
lfq usage --json | jq '.groups[0].tokens.input'  # raw JSON works
```

All existing Python tests pass. New tests cover token formatting and command output.

Advances wave goals: "Surface tokens inline at every level" and "Elevate Provider into a first-class concept."
