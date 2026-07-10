# Intelligence branch review

## What was implemented

This branch turns the local run journal into a queryable telemetry ledger. It
adds stable trace/span identity, closed event vocabularies, provider/model and
cumulative usage boundaries, then exposes that evidence through `lf doctor`,
`lf runs`, `lf trace`, and `lf usage`.

It also adds `lf tokens` for current and historical codebase weight, a Mac
Telemetry dashboard backed by the JSON CLI surfaces, and `lf wavechat` for
steering and observing a wave from one terminal pane. The gate pass tightened
mixed-provider spend attribution, span identity, continuity checks, dashboard
request ordering, tracked-symlink accounting, user docs, and the local UI gate.

## Key choices

- A run is the trace and a launched process is the span. `process_id` and
  `parent_process_id` make nesting explicit instead of reconstructing it from
  timestamps or names.
- Usage events remain cumulative provider boundaries. `own_spend` diffs them
  once, and both JSON and human summaries fold those same rows. This preserves
  correct attribution when one flow launches more than one provider or model.
- Migration 057 discards pre-contract `run_events`. Their identity and usage
  semantics were ambiguous; per-repository journals remain the source material
  for any deliberate future import.
- Telemetry stays local: Rust reads SQLite and Git directly, while the Mac app
  is a thin client of stable `lf ... --json` commands. No duplicate lfd HTTP
  surface was kept.
- Historical token measurements cache by blob SHA. A tracked symlink weighs its
  link text, matching Git's blob, so the current tree and history use identical
  semantics.
- The repository's Loopflow UI gate now mirrors CI's `build-for-testing` check.
  The hosted UI runner can exit before app bootstrap on this host; compilation
  is the merge-gate signal CI actually enforces.

## How it fits together

Agent and flow launches append canonical events to repository journals, which
are ingested into `run_events`. Rust query commands turn that ledger into
audits, trees, and spend boundaries; the Swift dashboard invokes their JSON
forms and renders the results without owning a second telemetry model.

`lf tokens` follows a parallel local path: tracked files and historical Git
blobs are tokenized with the context tokenizer, while `blob_tokens` avoids
repeating work across snapshots.

## Risks and bottlenecks

- Migration 057 intentionally clears old `run_events`. Applying it without the
  repository journals would lose historical queryability.
- A cold 365-day token history took about 2m07s on this repository. The cached
  92-snapshot query took 2.55s; dashboard first load can still be conspicuous.
- Human `lf usage` folds ledger boundaries in process. It is correct and small
  today, but latency should be watched as the event table grows.
- The dashboard requires the app bundle to contain the matching `lf` binary.
  `scripts/loopflow-dev.py run-debug` builds and installs that bundle correctly.
- The headless gate could not capture rendered dashboard states. Swift tests
  and the exact Xcode `build-for-testing` merge gate passed.
- Live Linear tasks were unavailable because the stored token has expired. The
  local wave goal, memory, and project definitions were reviewed instead.

## What's not included

- The eval-results harvester and longitudinal portfolio comparison.
- A PR delivery record or automatic movement metrics.
- Emission of the reserved `escalated` event.
- Pause, resume, and interrupt verbs in `lf wavechat`.
- Remote telemetry, hosted storage, or backfilling ambiguous pre-contract rows.

## Wave alignment

This directly advances the Trace project's reader-survival, single-home spend,
continuity, and one-query stats KRs on the long-lived ledger. It also gives the
Context project a measurable code and prompt-weight surface. Month-long proof,
run replay, and the Evals project's comparative harness remain open; this gate
establishes the readers and evidence contract they need rather than claiming
those time-based KRs early.

## Validation evidence

- `cargo fmt --all -- --check` and
  `cargo clippy --all-targets -- -D warnings` pass.
- The full repository matrix passed: Python (51), Rust, website (59 passed,
  3 skipped), Swift package (306), legacy Swift (5), and the end-to-end smoke
  test. The Loopflow app merge gate then passed all four suites with
  `** TEST BUILD SUCCEEDED **`.
- Post-polish focused Rust tests pass, including cumulative spend, mixed
  providers, continuity through the current tail, agent metadata replacement,
  span identity, and tracked symlinks.
- All six `lf doctor` checks are green on the long-lived ledger. Eight usage
  boundaries reconcile exactly: 20,010 JSON tokens equal the 20,010-token
  human total.
- At `43ac6999`, live and historical token measurements reconcile exactly at
  183,599 lines and 1,791,384 tokens. The year contains 92 snapshots.
