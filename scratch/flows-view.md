# Flows View (Concerto)

A reference screen for navigating loopflow's flows and steps. Answers "what does X do?" and "where is X used?" without leaving the app.

## Problem

The flow catalog has grown past what fits in one head — even the author's. Reading the README or grepping `builtins/flows/` isn't the right interaction when you're mid-session and just want to know whether `gate` runs inside `build`.

## Shape

Two trees, one selection.

**Left — Catalog.** Grouped by category (code, build, garden, vsm, algedonic, ops). Each flow expands into its steps. Steps that are themselves flows keep expanding. `xor` renders as sibling sub-trees under a diamond node. `loop` renders as a labeled container with the body indented.

**Right — Used by.** Recursive upward walk from the selection. Direct parents shown flat; each parent expandable into *its* parents, all the way up. Breadcrumb label on each path. Click any breadcrumb to re-select in the left pane.

Both panes live in a new top-level Concerto tab: **Flows**.

## Data

One `lfd` endpoint: `GET /flows`. Returns the resolved catalog as JSON.

```json
{
  "flows": {
    "build": {
      "category": "build",
      "source": "builtin",      // or "repo" for .lf/flows overrides
      "body": [
        { "kind": "step", "name": "kickoff" },
        { "kind": "step", "name": "review-design" },
        { "kind": "loop", "steps": [...], "exit": {...} },
        { "kind": "flow", "name": "deploy" }
      ]
    }
  },
  "steps": {
    "gate": { "category": "code", "source": "builtin" }
  }
}
```

The Rust engine already parses flows into a resolved model — reuse it, don't re-parse YAML on the Swift side.

"Used by" is derived client-side by walking the catalog once. Direct + transitive, both lazy: the right pane only expands a parent chain when the user drills in.

## Overrides

`.lf/flows/*.yaml` wins over builtins. When a flow exists in both, show the repo version with a subtle "override" badge so it's visible without being loud.

## Scope

- **In**: builtin + repo flows, builtin + repo steps, their composition.
- **Out (for now)**: waves, crons, triggers, directions, areas. Reference material only, not runtime state.

## Open

- xor path labels in the tree — show just the key (`act`, `silence`) or the description too?
- Search/filter across both panes — v1 or later?
- iOS layout — the two-pane model doesn't fit; probably a single tree with a push-nav for "used by". Defer until Mac v1 lands.

## From use

Catalog view helps — seeing `build` expand into its steps, and seeing what `gate` is used by, is the "can I hold this in my head" answer the README couldn't give. But using it surfaced two gaps that are the real next work.

**Are these the right built-in flows?** Placement is still shifting as we live with them (`s1-build` → govern, `sync` → ops). The catalog makes it easier to ask this question, but doesn't answer it. Expect continued tuning as flows get invoked in anger.

**Clarity during execution is missing.** The catalog is static structure; it doesn't show where you are in a running flow. The pain points, in order of severity:

- **Cold start.** Coming back to a session 15 min later, you can't reconstruct what was spec'd vs built vs deferred. No artifact tracks it.
- **XOR opacity.** At the CLI, xor flows branch silently — the router picks a path, the next step runs, no "chose `demo` because X" surfaces. You only notice branching happened by watching logs.
- **Position loss.** Linear flows have the same problem, quieter: which step am I on, what's next, what just happened?

The right shape for the next piece is a **session-state overlay** — same catalog tree, but rendered with current position, router decisions, and per-step status (built / in-flight / pending / deferred). CLI breadcrumbs (`[flow:build 3/6] maybe:demo ran`) as a down payment.

Scope for this PR stays at the static catalog. Session-state is a follow-up; capturing it here so it doesn't evaporate.

## `maybe` as a primitive

Three of the four xors in the catalog are `xor(X, silence)` — "do X, or don't." The fourth (`build`'s inner `xor(demo, code-review)`) is really two independent "do this aspect if applicable" decisions, not a genuine fork. None of them are "pick exactly one of these two real paths."

Introduce `maybe(step)` — a unary wrapper that conditionally runs its wrapped step.

| Before | After |
|--------|-------|
| `xor(build, silence)` | `maybe(build)` |
| `xor(garden-act, silence)` | `maybe(garden-act)` |
| `xor(s1-build, silence)` | `maybe(s1-build)` |
| `xor(demo, code-review)` | `maybe(demo) → maybe(code-review)` |

After this, xor likely has no users in the built-in catalog. Keep the primitive for genuine either-or branches, but the default "conditional" pattern is `maybe`.

Benefits:
- `silence` as a sentinel leaf disappears.
- Rendering: decorate the wrapped step with a `?` glyph or dashed border; no diamond + sibling sub-trees.
- Each decision is local and independent; no router arbitrating between paths.
- CLI breadcrumbs are trivial per-step: `[maybe:demo] ran` vs `[maybe:demo] skipped`.
- Parse/resolve simpler than xor.

Scope: next PR. This one stays at the reorg + static catalog view.
