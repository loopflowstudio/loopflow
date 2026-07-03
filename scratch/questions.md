# goals wave — open questions & blockers

## Tooling gap (2026-07-02): dispatch-through-lfd unavailable in headless loop

The operating prompt instructs dispatching work with `lfq worker run <wave>
--flow <flow> --task`, and MEMORY.md claims an `lfq sessions` / `lfq attach`
"session cockpit" shipped. Neither is true in this build:

- `lfq` has no `worker` / `sessions` / `attach` subcommands (only list/show/
  create/run/stop/delete/land/logs/usage/providers/auth/repos/token).
- `lfd` is not running here (`lfq list` fails to connect), so even the existing
  HTTP dispatch path (`POST /v0/waves/{id}/dispatch`) is unreachable.

Consequence: this loop iteration dispatches via the harness Agent tool instead
of a real tmux worker session. Follow-up for the wave: either wire the
`lfq worker`/`sessions`/`attach` CLI the operating prompt promises, or correct
the operating prompt + MEMORY.md to match the actual dispatch surface
(`lfq run` / lfd HTTP). Tracked so the contract and the tooling stop drifting.

## Executive decision (2026-07-02): wave-budget open question

`2-wave-budget.md` asks how much budget machinery lives in loopflow core vs. is
user-authored. Resolved to the item's stated lean: **ship a minimal hard
`spend_cap` + block→human pause in the core** (a 24/7 loop must not depend on
opt-in safety), and **expose the cost-to-date signal + pause primitive** so
richer budgeting can be written as goals on top. Build to that.
