# Goal primitive (wave/goals item 1)

First build of the `goals` chord. Full vision: `wave/goals/README.md`.
Decisions: `release/unreleased/DECISIONS.md` (2026-06-30).

## What to build

A `goal/` prompt primitive — a prompt run *in a loop* — resolved via the
standard `.lf/` override chain, plus a `goal:` reference on waves. A wave's loop
body runs its resolved goal prompt instead of a hard-wired flow. This is the
third primitive: **step** (run once), **flow** (composed), **goal** (looped).
It supersedes the already-removed `direction`.

## Data structures

```rust
/// A prompt run in a loop. The looping primitive.
#[derive(Debug, Clone, PartialEq)]
pub struct Goal {
    pub name: String,
    pub prompt: String, // markdown body; metric/target encoded in the prose
}

pub struct Wave {
    // ...existing fields...
    pub goal: Option<String>, // name of the goal this wave loops; replaces `direction`
}
```

## Key functions

```rust
/// Resolve via override chain: builtin -> ~/.lf/goals/ -> <repo>/.lf/goals/
/// (and `goal/`). Highest specificity wins. Mirrors step resolution exactly.
fn find_goal(name: &str, repo_root: &Path) -> Option<Goal>;

/// Render the per-iteration loop prompt. Context exposes available flows and a
/// roadmap handle (the seam Asana plugs into later) so the goal can
/// "decide next move -> dispatch a flow as inner work."
fn render_goal(goal: &Goal, ctx: &LoopContext) -> String;
```

The loop (currently hard-wired to `ship-roadmap`) reads `wave.goal`, renders it,
and runs it as the iteration body.

## Constraints

- Override precedence must match steps *exactly* — users learn one rule.
- Do not resurrect the `direction` field; `goal` is the looping intent.
- The render context must already expose flows + a roadmap handle, even though
  Asana wiring lands in item `2-asana-roadmap` — the seam must exist now.
- A goal is a *prompt*, not config. The metric/target lives in the prose
  ("drive auth flake to zero, here's how I'll know"), not a struct field.

## Done when

- `goal/<name>.md` in a repo overrides a builtin goal of the same name
  (test mirrors the step-override test).
- A wave with `goal: <name>` runs that goal's prompt as its loop body, verified
  by a smoke test where one iteration executes the goal prompt.
- `cargo test` passes; `cargo fmt` + `cargo clippy -- -D warnings` clean.

## Measure

Not quantitative — this is a primitive + wiring change. Coverage is proven by
the override test and the loop-body smoke test, not a benchmark.

---

# Added later — address after the initial primitive lands

Doesn't block the primitive. Pick it up once `goal/` resolves, round-trips, and
drives a loop iteration.

## Looping agents delegate; they don't hand-write code

The **LOOPFLOW operating prompt** (the universal Wave orchestration contract
woven into every looping session's initial prompt — see `wave/goals/README.md`)
must instruct the looping, goal-seeking agent to **avoid writing code itself**.
Its job is orchestration, not implementation: read roadmap/metrics → pick the
next move → **dispatch an `lf` flow or `lf` step** that hands the actual edits to
a subagent. Writing code inline is the exception (a trivial fix not worth a
subagent), never the default.

Beyond fire-and-forget dispatch, the looping agent can hold **interactive
sessions** with its subagents — using the interactive agent APIs to open a
session, steer it, answer its questions, and read its results back, then carry
that into the next move. The loop "communicates" with subagents through these
sessions rather than doing the work in its own context.

The loop is the *head*, the flow is the *hands*: keep the loop's transcript about
*decisions*, and let scoped subagents own the diffs.
