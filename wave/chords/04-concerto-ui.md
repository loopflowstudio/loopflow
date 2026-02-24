# 04: Concerto UI

Surface chords in Concerto — creation, voice management, and execution visibility. The macOS app should make chord structure obvious and chord operations feel natural.

## What exists after this

Concerto shows chords as expandable groups in the wave list. You can see child voices, their status, and execution progress. Creating a chord (via join) and managing voices (join/leave) works from the app. Chord structure is visually distinct from solo waves.

## What Phase 01–03 established

The data model (Voice/Chord enum), execution engine (parallel child execution with inherited stimulus), and listen step (inter-voice communication) are all server-side. Concerto currently shows waves as a flat list — it has no awareness of parent/child relationships or chord structure.

## Key questions

- How do chords appear in the wave list? Inline expandable, or a separate "chord view"?
- What does chord execution look like in real-time? Per-voice progress, or aggregate?
- How does join/leave map to UI gestures? Drag-to-group? Multi-select + action?
- Should nested chords (chord containing chord) be visible, or collapsed?

## Done when

- Chords are visually distinct from solo waves in the wave list
- Child voices are visible within their parent chord
- Chord execution status shows per-voice progress
- Join and leave operations are accessible from the UI
- Nested chord structure is navigable
