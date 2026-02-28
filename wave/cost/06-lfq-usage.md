# 06: lfq Usage

**Finish line:** `lfq usage --wave engbot` prints token summary to terminal. `lfq providers` lists providers with models and auth status.

## What to build

```bash
lfq usage                    # global summary
lfq usage --wave engbot      # per-wave
lfq usage --model opus       # per-model
lfq usage --step implement   # per-step
lfq usage --prompt           # prompt composition view
lfq usage --from 2026-02-01  # time-filtered
lfq usage --billing          # split by auth type (subscription vs apikey)
lfq providers                # list providers with auth status and models
```

Reads from the usage APIs. Tabular terminal output, composable with other shell tools.

Backend is ready: `GET /v0/usage/summary` returns flat aggregates (group_by wave/flow/step/model), `GET /v0/usage/timeseries` returns time-bucketed groups (day/week/month). Both share a `ValidatedUsageQuery` pipeline. `cost_usd` is populated on TurnUsage for OpenCode sessions at ingestion time.

`lfq providers` reads from `GET /v0/providers` (Phase 03). `lfq auth zen` connects OpenCode Zen — the broker is ready, just needs a CLI subcommand.

### Auth-type-aware billing

When `--billing` is passed (or when any provider uses API key auth), split output into subscription and metered buckets:

```
Subscription (OAuth)          Metered (API key)
─────────────────────         ──────────────────────
Claude   142k tokens          Codex   89k tokens ~$4.20
                              ─────────────────────────
                              total metered: ~$4.20
```

Dollar estimates use `CostRates` from the model registry. Only shown for API-key sessions — subscription sessions show token volume only.

Per-provider auth type comes from the auth wave's credential type field in the DB. The usage API includes `auth_type` per turn so cost computation is accurate even when a user switches mid-session.

