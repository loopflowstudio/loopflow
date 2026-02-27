# Gate Review: cost inline views ingest

## What was done

Ingested wave item `wave/cost/04-inline-views.md` into an elaborated design doc at `scratch/cost-inline-views.md`. The design covers inline token views for Concerto — surfacing token consumption at every level of the UI hierarchy via progressive disclosure.

Also deleted the entire `wave/tmux/` wave (README, yaml, remaining 05-polish item).

## Key choices

**Design doc scope is well-bounded.** This sprint covers: backend RepoId on sessions, SessionFilters extensions, Swift models + SSE parsing, SessionState accumulation, UsageService HTTP client, token formatting, and transcript per-turn usage view. Portfolio card, WaveRunRow badge, and flow pills are explicitly deferred to next sprint.

**Live vs historical split.** Active sessions get real-time counts from SSE events in SessionState. Completed sessions/waves fetch from HTTP usage endpoints. Avoids replaying event history client-side.

**Flat token counts, not cost.** v0 shows token volume. Dollar costs deferred to Phase 05. Clean separation — UI renders UInt64 now, ready for costUSD later.

## How it fits together

Backend (Rust) denormalizes RepoId onto SessionConfig and extends SessionFilters. Swift layer adds TurnUsage/ContextSnapshot/TokenTotals models, parses new SSE events, accumulates in SessionState. UsageService protocol provides HTTP client for historical data. Transcript view renders per-turn usage inline.

## Risks and open questions

**Tmux wave closure is out of scope.** The `wave/tmux/` deletion (README.md, tmux.yaml, 05-polish.md) is unrelated to cost wave work. Consider moving it to a separate branch or commit with its own rationale. The remaining tmux item (interactive test coverage) is being dropped without explanation.

**No code yet.** This is a design-only change. The gate is effectively: "is the design doc ready for implementation?" Yes — it has concrete types, API shapes, implementation order, and testable done criteria.

## What's not included

- No implementation code (this is the ingest/kickoff output)
- Portfolio card, WaveRunRow, flow pills (next sprint)
- Dollar cost display (Phase 05)
- No changes to wave/cost/README.md or cost.yaml to reflect item status
