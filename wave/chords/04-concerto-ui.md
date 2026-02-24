# 04: Concerto UI

Surface chords and listen relationships in Concerto. The macOS app should make wave grouping and inter-wave wiring visible and manageable.

## What exists after this

Concerto shows chords as named groups in the wave list. Waves within a chord are visually grouped. Listen stimulus relationships are visible — you can see which waves react to which. Creating chords and managing membership works from the app.

## What Phase 01–03 established

Waves are flat structs. Chords are named groups (from the `chords`/`chord_members` tables). Listen stimuli wire waves to react to each other via `source_wave_id`. Concerto currently shows waves as a flat list with no awareness of chord grouping or listen wiring.

## Key questions

- How do chords appear in the wave list? Section headers? Collapsible groups? Tags?
- How are listen relationships visualized? Arrows between waves? A separate "connections" view?
- Should chord creation/membership be drag-and-drop, or menu-driven?
- How do ungrouped waves (not in any chord) appear relative to grouped ones?

## Done when

- Chords are visually distinct groups in the wave list
- Member waves are visible within their chord
- Listen stimulus relationships are indicated (which wave listens to which)
- Chord CRUD (create, delete, add/remove member) is accessible from the UI
- Ungrouped waves display cleanly alongside chord groups
