---
status: in_progress
seq: 1
source: wave/loop/01-foundations-live-state.md
---

# Foundations + Live PR State

## Goal

Make stacked wave lineage explicit and make live GitHub PR state authoritative for queue/API behavior.

## Why this matters

Stacked runs currently break when PR state changes outside daemon control (manual merge/close/draft changes). Step 02 queue automation depends on deterministic ancestry and current PR truth.

## Decisions locked

- Run lineage is explicit (`parent_run_id`, `parent_pr_number`, `stack_position`, `stack_group_id`, `stack_status`).
- Historical backfill must never overwrite explicit lineage written by newer code.
- Live PR cache (`live_pr_states`) is authoritative for current state; snapshots remain historical only.
- Degraded GitHub sync is surfaced explicitly via stale flags (no silent fallback to "open").

## Current implementation state

Covered in this branch:

- Migration guardrails preserve explicit lineage during backfill (`005_wave_run_lineage_live_pr_state.sql`).
- Store transition tests cover `find_next_unmerged_run` across `open/merged/closed/unknown` live states.
- Route/projection tests verify `open_pr_count` comes from live PR state (not snapshot state).

## Remaining work for this wave item

- Verify runtime sync triggers/cadence behavior end-to-end (polling + refresh paths), not just projection/store semantics.
- Ensure queue-facing endpoints and consumers tolerate persistent stale-state signaling in no-token environments.
- Keep enum/storage mapping for `run_kind` safe from drift so migration assumptions stay valid.

## Risks to watch

- Migration backfill depends on `run_kind = main` filtering; schema/enum drift could silently affect lineage inference.
- Temporary sqlite files in route tests can be left behind on interrupted runs (low risk, but cleanup hygiene).
- Stale flags may be noisy in deployments without GitHub credentials.

## Done when

- `cargo test --all` proves:
  - iteration N links to N-1 and increments `stack_position`
  - backfill does not overwrite explicit lineage
  - merged/closed/unknown live PR transitions project correctly
  - `open_pr_count` is live-state derived
- `GET /waves/{id}` reports accurate `open_pr_count`, `stack_count`, and `has_stale_pr_state`.
- `GET /wave_runs?wave_id=<id>&order=stack` returns stable oldest-first lineage with per-run stale flags.
