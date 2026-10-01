# Open design questions

2026-10-01 — no unresolved product decisions block the accepted scope.

Jack Heart accepted the design on 2026-10-01: **Create Task** launches the
single **capture-task** skill from repository or Wave scope. Exploration adapts
to the idea; filing happens once intent is clear. The conversation remains at
repo/Wave scope for additional Tasks. Existing Wave conversations share capture
behavior; owners operate captured work; `design` stays available.

Jack Heart subsequently authorized Product placement and launch on 2026-10-01.
Product's recommended `feature` Flow preserves kickoff and review boundaries.

## Kickoff assumptions (2026-10-01)

Made without Jack Heart's confirmation; each is reversible.

- With a Project selected, Create Task uses that Project's Wave. The accepted
  design names only overview, Wave and Task.
- When the selected Wave's name cannot be resolved, Create Task opens at
  repository scope instead of disabling itself.
- `wave/session`'s "Develop direction" section is replaced by a pointer to
  `capture-task`. This drops its instruction to write ideas under `scratch/`
  and its "once the user has agreed" filing condition in favour of capture's
  "file when intent is clear, honor existing authorization". The accepted
  design says Wave conversations share capture; it does not mention this
  section by name.
- The Wave menu action appears on Wave rows only. Task rows reach capture
  through the visible button.
