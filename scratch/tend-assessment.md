# Tend Assessment — 2026-03-17

## Summary

The redesign chord is in early bootstrap — one large foundational commit on main, one branch building the first tend cycle. All energy is concentrated in chord-model, which is correct: tend must work before anything else matters. The key tension is that three of four waves are completely silent with no items in flight, while the critical path (chord-model/02) has a finished design doc but zero implementation commits.

## Wave: chord-model
**Health**: steady
**Evidence**: PR #560 landed a substantial bootstrap — chord CRUD removal (600+ lines deleted), all five tend step prompts, flow YAML definitions, wave configs, migration 028. The current branch has a completed design doc for item 02 (tend flow steps). The design is specific: update scan-waves to read `lfq show --json`, add Rust flow tests for tend structure, validate first cycle. No implementation code yet, but the design is tight enough to execute directly.
**Pressure**: The gap between "design complete" and "implementation started." The design doc has been done since this branch was created, but the branch diff shows prompt work and scratch artifacts, not Rust tests or scan-waves updates. The next commit should be code.

## Wave: clear-the-deck
**Health**: silent
**Evidence**: No branch, no PR, no items in flight. All four items are independent and unblocked. PR #560 already did a clear-the-deck-adjacent action (chord CRUD removal — 434 lines of routes, 200+ lines of store code). PR #559 touches provider auth, which is adjacent to item 01 (auth consolidation).
**Pressure**: Nothing is blocking this wave — it's silent by neglect, not by design. Four independent deletion items are the lowest-risk work in the chord. Each could ship as a single PR with high confidence. The wave is leaving easy momentum on the table.

## Wave: agent-embedding
**Health**: blocked
**Evidence**: No items in flight. Item 01 (block queue view) depends on signals/01 (block taxonomy) — can't display blocks that don't exist yet. Item 02 (terminal embedding) has research risk (Ghostty in SwiftUI is unproven). PR #559 is 10 days old with green CI — foundation work for the conductor pivot that's just sitting there.
**Pressure**: The stale PR. PR #559 is the only open PR across the entire chord and it's aging. Landing it would clear the decks for this wave and resolve a dependency for clear-the-deck/01 (auth consolidation, which benefits from understanding the auth surface PR #559 touches). The block queue dependency on signals/01 is real but further out.

## Wave: signals
**Health**: silent
**Evidence**: No branch, no PR, no items in flight. Block taxonomy (01) is a dependency for agent-embedding/01. No commits on main specific to signals items.
**Pressure**: Sequencing. The roadmap puts signals/01 in Phase 1 alongside chord-model/02 and chord-model/03. But signals/01 (block taxonomy) doesn't depend on the tend flow — it's a data model and API. It could start now, in parallel with chord-model/02 implementation, and unblock agent-embedding sooner.

## Chord-Level
**Balance**: Severely imbalanced. Chord-model has all the activity — one shipped PR, one active branch, a completed design doc. The other three waves have zero activity. This is expected during bootstrap (tend must exist before it can tend), but the imbalance is sharper than necessary. Clear-the-deck and signals/01 could both be producing work right now.

**Gaps**: PR hygiene. PR #559 is 10 days old with green CI and no wave owns landing it. It touches both agent-embedding area (Concerto) and lfd provider auth (clear-the-deck area). Someone should land or close it — it's accumulating review staleness for no reason.

**Phase**: Phase 1 (Bootstrap) is correct. The roadmap says: chord-model/02, then signals/01, then chord-model/03. The sequencing is right, but "then" should be "and" for the first two — they're independent and could run in parallel.

## Pressure Points
1. **Ship chord-model/02 implementation.** The design doc is complete. The implementation is three concrete tasks: update scan-waves.md prompt, add two Rust flow tests, validate the tend flow loads. This is the critical path for the entire redesign — every other wave's autonomy depends on tend working. Convert design to code.
2. **Land PR #559.** Ten days old, CI green, no reviewer activity visible. It's foundation work that unblocks understanding of the auth surface (relevant to clear-the-deck/01) and establishes Concerto patterns (relevant to agent-embedding). Aging PRs are a signal that review cadence isn't matching production cadence.
3. **Start clear-the-deck in parallel.** Four independent items, zero dependencies on other waves, each a single PR. This wave exists specifically to generate early momentum and reduce surface area. Keeping it silent while chord-model bootstraps wastes a parallel lane. Even one item shipped (auth consolidation or growth cleanup) would demonstrate that the chord can coordinate multiple active waves.
