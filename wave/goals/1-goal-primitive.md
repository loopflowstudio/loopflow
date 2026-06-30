---
priority: urgent
---

# Goal primitive

**Finish line:** `.lf/goals/<name>.md` resolves and overrides a builtin goal, a
wave always has a `goal:`, and one loop iteration runs that goal's prompt as its
body instead of a hard-wired flow.

## Context

`direction` was already removed as machinery (2026-06-19; wave model is
`area × flow`). A **Goal** is its reincarnation as the *looping, measurable*
form: a prompt run in a loop. It becomes the third prompt primitive — step (run
once), flow (composed), **goal (looped)** — and the product developer's primary
authoring surface. Everything else in this wave speaks it, so it's first.

## What to shape

- A goal primitive resolved from `<repo>/.lf/goals/`, `~/.lf/goals/`, then
  builtins. Singular `.lf/goal/` and repo-root `goal/` are not accepted.
- A required `goal: String` reference on the wave, defaulting to
  `ship-roadmap`; patch/update payloads may still use optional `goal` to mean
  "leave unchanged."
- The loop body reads `wave.goal`, renders it, and runs it — replacing the
  hard-wired `ship-roadmap`. The render context must expose available flows and
  a roadmap handle (the seam Asana plugs into later), so the goal prompt can
  "decide next move → dispatch a flow as inner work."

Full design doc for this item: `scratch/jack-heart.wave-looping-agents.md`.

## Done when

- A repo `.lf/goals/<name>.md` overrides a builtin goal.
- Legacy `.lf/goal/<name>.md` and repo-root `goal/<name>.md` do not resolve.
- A wave with `goal: <name>` runs that goal's prompt as its loop body (smoke
  test: one iteration executes the goal prompt), regardless of the run snapshot
  flow.
- `cargo test` passes.
