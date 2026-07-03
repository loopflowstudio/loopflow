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

## Reduction pass (compress)

Reshaped `run_pass` in `loop.rs`: dropped the `PassOutcome::SpawnError` variant
and let setup failures (spawn, pipes, log I/O, wait) propagate as `Err` via `?`.
Cut ~17 lines of manual match-and-rewrap; the outcome enum now models only what
a pass that actually *ran* can produce. Also deduped the `wave/<name>` path join.

**Observed, left out of scope:** `goal.rs::launch_goal_batch` duplicates the
headless-launch sequence in `run.rs` (check_cli → write prompt/context logs →
`StreamFormat::Human` → `launch_agent` → exit-code hint). Collapsing it means a
shared `engine::agent` helper and rewiring `run.rs`, which this branch didn't
touch. Worth a dedicated pass once a third caller appears.

## Gate pass: roadmap lives in Asana

Removed the branch-added `wave/goals/*lf-loop*.md` roadmap files and the two
duplicate-project `asana_id` additions from existing wave goal files. Current
loopflow guidance says Asana is the roadmap source of truth; local numbered
roadmap mirrors should not be extended from this branch.

Assumption: the `lf wave` follow-on roadmap should be filed or adjusted with
`lf op pm update` against the canonical `wave/goals/GOAL.md` Asana project when
the roadmap owner is ready to mutate external project state. Gate did not call
Asana write APIs.
