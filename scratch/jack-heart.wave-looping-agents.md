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
