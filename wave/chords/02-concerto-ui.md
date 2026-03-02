# 02: Concerto Chord UI

**Finish line:** Concerto shows chords as named sections in the wave list. Waves within a chord are visually grouped. Ungrouped waves display cleanly. Creating chords and managing membership works from the app.

## Context

Signal cleanup has landed — `wave.mode` (Loop/Cron/Manual) is on the wave struct, which affects how waves display in the sidebar. Trigger rename has landed across the full stack (Rust, Python, Swift, docs, wave config). `Stimulus`/`stimuli` fully replaced with `Trigger`/`triggers` including Swift models, services, views, and tests.

FlowRun container (01) should land before or alongside this — it adds `primary_flow_run` and `triggered_flows` to WaveRun, which feeds the per-iteration detail view. Chord CRUD already works end-to-end from the HTTP API and Python client.

## Key questions

- How do chords appear in the sidebar? Section headers with collapsible groups seems natural — flat when one chord, sections when multiple (progressive disclosure per README goal).
- How are listen relationships indicated? Subtle wiring indicator between waves, or a separate connections view?
- Should chord membership be drag-and-drop, menu-driven, or both?
- How do ungrouped waves (not in any chord) appear relative to grouped ones?

## What to build

- Sidebar sections for chords — flat list when only the default chord exists, grouped sections when multiple chords
- Visual grouping of member waves within their chord section
- Wave trigger indicator (which wave listens to which)
- Chord CRUD from the UI: create chord, delete chord, add/remove wave from chord
- Clean display for ungrouped waves alongside chord groups
- Branch flow visualization — show which path was taken, routing verdict, cycle status for QA->fix->deploy loops
- Human-in-the-loop branch selection — allow manual path override from the UI
