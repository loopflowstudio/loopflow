# Open questions — jack-heart.lf-loop

## Rebase onto main: duplicate Asana bootstrap (resolved by best judgment)

Both `main` and this branch's `lf pm: bootstrap goals` commit independently
bootstrapped `wave/goals/` into Asana, producing two parallel projects:

- **main:** project `1216257471889000` (referenced by `wave/goals/GOAL.md` on trunk)
- **this branch:** project `1216272792262792`

Every conflict during rebase was a divergent `asana_id`/`asana_project` pointer,
not substantive content. Resolution taken:

- **Adopted main's project + task IDs everywhere** (took `--ours` on all
  conflicts). Rationale: main's project is on trunk and is what persists after
  merge; a rebasing branch conforms to trunk.
- `wave/goals/GOAL.md` — took main's version wholesale. Main's body is the
  richer, current one; the branch side carried the old `mode:`/`metrics:`
  frontmatter schema that main deliberately dropped.
- `wave/goals/README.md` — accepted main's deletion. Main restructured
  `wave/goals/` into concise `GOAL.md` + numbered milestone files; the branch's
  README edits were the old expression of the same `lf loop` content, which now
  lives in the numbered goal files.
- **Stripped `asana_id` from the three new lf-loop goal files**
  (`1-lf-loop-progress.md`, `2-lf-loop-chat.md`, `2-loop-crons.md`). They were
  registered in the abandoned duplicate project, so their IDs pointed at a
  different project than the wave's GOAL.md. Blanking them leaves the wave
  internally consistent (one project) and lets `lf op pm` re-register them into
  the canonical project.

**If this is backwards** — i.e. project `1216272792262792` is the one you want
live — reset `wave/goals/GOAL.md`'s `asana_project` and the eight existing
`asana_id`s to the branch values, and restore the three stripped IDs. But run
`lf op pm` to re-register the lf-loop goals in whichever project you keep.

## Implement pass scope: lf wave progress arm first

The design calls for the supervisor, monitor, cron, and chat arms in one branch.
This implementation pass took the smallest shippable slice: rename the foreground
progress command to `lf wave` (with `lf loop` as an alias) and retain each
bounded pass's stdout/stderr under `wave/<name>/streams/` for the future monitor
arm. Cron scheduling, monitor summarization, and in-process chat API remain
separate follow-on work.
