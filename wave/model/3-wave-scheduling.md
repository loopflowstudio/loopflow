---
asana_id: '1213883255344337'
notion_id: 333f8f99-3d81-8164-abe4-d80f2062ca44
---
# Wave Scheduling — Loops, Crons, and `parent` Replacing `mode`

**Finish line:** Waves are scheduled by `loops` and `crons`. The `mode` field and standalone `flow`/`workers` fields are gone. A wave is: loops + crons + triggers, all optional, all carrying their own flow.

## Context

After shipping wave crons, the shape of a wave reveals redundancy. `mode` (`loop` vs `manual`), `flow`, and `workers` are entangled — `workers: 0` makes `flow` vestigial, `mode: manual` is meaningless without workers, and crons already bring their own flow. The scheduling model should be three orthogonal mechanisms, not a bag of fields that conditionally depend on each other.

The insight: a loop is a cron that fires immediately and repeats on completion. A cron is a scheduled burst. Both support concurrency via `workers`. The difference is only the trigger — continuous vs time-based.

## The model

```yaml
# Continuous workers
loops:
- flow: build
  workers: 2

# Scheduled bursts
crons:
- flow: build
  workers: 3
  schedule: "0 9 * * 1"    # weekly sprint

# Maintenance (single worker by default)
crons:
- flow: garden
  schedule: "0 0 * * *"

# Cron-only wave — no loops needed
crons:
- flow: govern-coordination
  schedule: "0 0 * * *"
- flow: govern-identity
  schedule: "0 0 * * 0"
```

`workers` defaults to 1 on both loops and crons. All three scheduling mechanisms (loops, crons, triggers) are optional.

### Workers > 1 design contract

Flows are suitable for `workers > 1` when they start with a coordination step (like `ingest`) that allows multiple agents to fan out across different problems. The `ingest` step picks work, so N workers hitting it concurrently naturally avoid overlap.

Flows without coordination (like `garden`) are single-track — multiple workers would produce redundant runs. The system doesn't enforce this, but it's the design contract: `workers > 1` means "this flow has a coordination mechanism at the front."

## Fields removed

| Old field | Replacement |
|-----------|-------------|
| `mode: loop` | `loops:` entry |
| `mode: manual` / `mode: flow` | `parent:` (wave reference or None) |
| `flow:` (top-level) | `loops[].flow` or `crons[].flow` |
| `workers:` (top-level) | `loops[].workers` or `crons[].workers` |

`mode` was conflating two things: scheduling (loop vs one-shot) and lineage (self-initiating vs parent-initiated). Scheduling moves to `loops`/`crons`. Lineage becomes `parent` — a wave reference or None. Root waves have no parent. Child waves point at whoever spawned them.

## Migration

1. Waves with `mode: loop` → create a `loops` entry with the existing `flow` and `workers`
2. Waves with `mode: manual` and `workers: 0` → leave as-is (no loops, no crons unless already defined)
3. Waves with `mode: manual` and `workers > 0` → create a `loops` entry (these were effectively looping anyway)
4. Existing `crons` entries gain `workers: 1` default
5. `mode` column replaced by nullable `parent` column (wave ID reference)
6. Drop top-level `flow` and `workers` columns

## Done when

- `loops:` and `crons:` with `workers` work in wave YAML and API
- Top-level `mode`, `flow`, `workers` removed from schema
- Migration backfills existing waves into new shape
- Rust/Python/Swift models updated
- `workers > 1` on crons creates concurrent activations
