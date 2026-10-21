# v0.14.14

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.14 makes Flows behave the same however you start them, and stops finished work from asking for your attention. A Flow's steps now run headless by default even when launched from a terminal, matching what Task runs already did. Done and abandoned Tasks now read as clear rather than blocked by old failed runs, and Desktop's Wave inventory lists only current work. Older versions keep working against the store; this release has no compatibility break.

## Flows run headless unless you ask for a terminal

Before this release, a standalone `lf flow` started from a terminal opened every step as an interactive conversation, while Task runs already defaulted to headless. Whether a Flow stopped for you depended on how you launched it. You take part in a Task at its Workflow nodes, in the Task conversation, not inside the Flow steps between them, so every Flow now runs its steps headless. (#1628)

- One rule now applies to standalone Flows, Task edge runs and commands like `lf ship`: each step gets `--batch` unless you pass `-i`.
- `lf -i flow my-flow` opens each step in the native agent conversation and advances when you exit it.
- `-b` is no longer needed for Flows. `docs/lf.md` and `docs/authoring.md` describe the new default.

## Finished Tasks read clear

A done or abandoned Task with nothing still running now has condition `clear` ("Task is complete" / "Task is abandoned"). Before, a failed run in its history or a missing worktree could make it show `blocked`. Tasks whose execution is still unresolved keep their current condition, so a Task canceled upstream with unfinished local work stays visible until someone deals with it. (#1621)

- **Desktop inventory shows current work only.** The Owned/Unowned inventory in a Wave now leaves out settled Tasks unless their execution is still unresolved. Settled Tasks stay reachable from Project history and the sidebar. Paused Tasks drop out of the sidebar.
- **One definition of "current".** The Wave inventory and the completed-history filter now agree on which Tasks count as current.

## Realistic lifecycle data for testing

`lf-world build lifecycle <dir>` builds a `release` Wave from real `lf` operations. It includes completions 1, 5, 12 and 40 days old (one with its checkout removed), a paused Task, an abandoned Task, a Task canceled upstream with started work, a lost checkout, one Task in progress, one not started, and two standalone Tasks with no Wave. Rust and Swift tests now check the CLI and Desktop against this world, so drift between them shows up in tests rather than in use. (#1621)

- Swift world tests share one `BuiltWorld` fixture that runs `lf-world` and the source `lf` in an isolated environment.
- `TESTING.md` now builds `lf-world` alongside `lf` before the Swift tests.

## Operational notes

- **Interactive Flow steps need `-i`.** If you relied on a bare `lf flow my-flow` from a terminal to open each step interactively, run `lf -i flow my-flow` instead.
- **Not yet exercised end to end.** Neither PR's suggested walkthrough was performed: comparing `lf -a claude flow my-flow` with `lf -i -a claude flow my-flow` from a terminal (#1628), and pointing Desktop at a built `lifecycle` world to check the inventory and 7-day history (#1621).

## Small changes

- `scripts/disk-cleanup.sh` runs `uv cache clean`/`prune` with `UV_LOCK_TIMEOUT=5`, so cleanup gives up after a few seconds instead of hanging when another uv process holds the cache lock. (#1628)