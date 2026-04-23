# Session-State Overlay

**Finish line:** a reviewer landing on a running wave can answer "which step is running, what came before, and why" from Concerto (or CLI breadcrumbs) without opening logs.

Static catalog is the map; this is the you-are-here dot. The pain points, in priority order:

- **Cold start.** Coming back to a session 15 minutes later, there is no artifact that reconstructs what was spec'd vs built vs deferred. The logs scroll past and the diff doesn't carry flow position.
- **XOR opacity.** At the CLI, xor flows branch silently — the router picks a path, the next step runs, no "chose `demo` because X" surfaces. Branching is only visible by watching logs.
- **Position loss.** Linear flows have the same problem, quieter: which step am I on, what's next, what just happened.

## Shape

Same catalog tree as the static Flows view, but rendered with live state per step:

| State | Meaning |
|-------|---------|
| built | Step completed, produced its artifact |
| in-flight | Currently running |
| pending | Not yet reached |
| deferred | Skipped by a router decision or `maybe` condition |
| failed | Errored, human attention needed |

Router decisions surface inline — `[xor:act/silence → act] because <one-line rationale>` — wherever the path was taken. Same for `maybe(step)` once that primitive ships: `[maybe:demo] skipped — no user-visible change`.

## Down payment: CLI breadcrumbs

Before the full Concerto overlay, emit structured breadcrumbs on every step transition:

```
[flow:build 3/6] kickoff → review-design → code → xor:(demo|code-review)
[xor:demo/code-review → demo] chose demo: observable UI change shipped
[flow:build 4/6] maybe:demo ran
```

One line per transition, cheap to add, immediately useful in logs and CI output. The Concerto overlay reads the same structured events — no second telemetry path.

## Scope

- **In**: one live session in one wave, full tree with live state, router rationale at decision points, CLI breadcrumbs.
- **Out**: cross-session history, timeline replay, run comparison. Session-state first; history after we know what shape we actually want.

## Open

- Does "deferred" survive across `maybe` branches, or does the skipped subtree just not render? Probably render dimmed — absence is louder than greyed-out.
- Router rationale: always include the one-line reason, or only on demand? Always, with a character cap — an opaque decision is worse than a verbose one.
- Live updates via existing session SSE, or a new `/catalog/session/:id` endpoint? Prefer reusing the session stream.
