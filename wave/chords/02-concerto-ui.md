# 02: Concerto Chord UI

**Finish line:** Concerto shows chords as named sections in the wave list. Waves within a chord are visually grouped. Ungrouped waves display cleanly. Creating chords and managing membership works from the app.

## Context

Signal cleanup (01) should land first — it establishes WaveMode on the wave struct, which affects how waves display in the sidebar (loop vs. once badge/indicator). Chord CRUD already works end-to-end from the HTTP API and Python client.

## Key questions

- How do chords appear in the sidebar? Section headers with collapsible groups seems natural — flat when one chord, sections when multiple (progressive disclosure per README goal).
- How are listen relationships indicated? Subtle wiring indicator between waves, or a separate connections view?
- Should chord membership be drag-and-drop, menu-driven, or both?
- How do ungrouped waves (not in any chord) appear relative to grouped ones?

## What to build

- Sidebar sections for chords — flat list when only the default chord exists, grouped sections when multiple chords
- Visual grouping of member waves within their chord section
- Listen stimulus indicator (which wave listens to which)
- Chord CRUD from the UI: create chord, delete chord, add/remove wave from chord
- Clean display for ungrouped waves alongside chord groups
