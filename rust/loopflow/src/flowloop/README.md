# flowloop — the runtime for everything agentic

A **flowloop** is a looping flow: `clarify → pursue_goal → mutate`, run
again and again. The agent has **write access to a termination bit** —
the merged PR, the completed KR set — and its skill states exactly how to
decide when to set it. At the end of each pass the runner checks the bit and
exits the loop when it reads set. Self-report counts for nothing; only the
bit does.

```
lf task <linear-item-id>     # run one roadmap task to a merged PR, bounded
```

Every tier is the same shape, differing only on what it owns and what halts
it:

| Tier | Owns | Pass flow | Oracle |
|---|---|---|---|
| **wave** | the objective (`GOAL.md`) | `wave-pass` | `Never` — the loop is the point |
| **project** | a KR set (Linear items labeled `kr`) | `project-pass` | `KrSetDone` — every KR completed |
| **task** | one design doc → one small PR | `task-pass` | `PrMerged` — `gh pr view` says MERGED |

## Layout

- `mod.rs` — `Tier`: the tier → pass-flow → oracle binding.
- `pass.rs` — one bounded, headless run of a tier's three-phase flow in a
  worktree (`lf -b <tier>-pass`, killed on timeout).
- `oracle.rs` — the halt predicates: PR state via `gh`, KR completion via
  the wave's Linear project (an empty KR set refuses to start), `Never`.
- `run.rs` — `FlowloopRun`: the registry-backed run lifecycle (worktree,
  store row, status) shared by the task and project drivers.
- `task.rs` — `lf task`: sequential driver — pass → poll → wait-or-pass →
  caps. Waiting ≠ thrashing: a clean tree with an open PR sleeps instead of
  re-passing. Caps escalate via `lf chat --parent` and exit nonzero.
- `project.rs` — same driver shape over a KR set. Built, not yet wired to a
  CLI verb — the wave spawns projects when the tier is wired.
- `wave.rs` — the wave driver: the residency's event scheduler (inbox /
  heartbeat / cron, biased select) where each wake runs one `wave-pass`
  child. See `wave/README.md` for the listener/resident topology.

Tier behavior lives in the **skill texts**, not runtime branching: the nine
builtin steps (`wave_clarify` … `task_mutate`) under
`engine/builtins/build/step/` state each tier's artifact, move menu, and
oracle. Evolving a tier = editing its skills, no code change.

Phase runs are plumbing — never surfaced in the product. Chat is the one
interface to a flowloop; only execs surface as attachable sessions.
