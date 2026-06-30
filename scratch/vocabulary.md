# Vocabulary: Loop / Worker / Flow

A draft to react to — not a decision. Works Jack's sketch end-to-end, maps it
onto today's concepts, resolves the two tensions, and lists what a rename would
actually touch.

## The sketch

> Loop = Loop Prompt = Goal. Flow = work atom. Loops are always-running,
> interactive/interrupt/steerable sessions that generate and monitor "worker"
> subagents that perform loopflow flows.

## Proposed stack

```
Loopflow      the product / surface — where you watch and steer Loops
  └─ Loop     an always-running, steerable session, directed by a Goal (Loop Prompt)
       └─ Worker   a subagent the Loop spawns and monitors
            └─ Flow    a goal loop's grain — dispatched whole, never cracked open
──────────────────────────────────────── grain line (Loops dispatch Flows) ────
                 └─ Step   the internal atom — one prompt → one agent action
  lf          the engine that executes Steps (the internals)
```

It reads in the name: **Loop** + **flow**. And it lines up with the existing
"waves as looping systems directed by Goal prompts" intent (the goals chord).

## Map onto today

| Today | Proposed | Notes |
|-------|----------|-------|
| Wave (mode `loop`) | **Loop** | the always-on, steerable thing |
| Wave's goal / former `direction` | **Goal** / **Loop Prompt** | what directs the Loop |
| pool `workers` | **Worker** | already the word; promoted to a first-class noun |
| Flow (chain of steps) | **Flow** | structurally unchanged; the grain a goal loop dispatches (atomic-ish) |
| Step (prompt → agent) | **Step** | unchanged — the internal atom (what `lf` executes) |
| the product / Concerto app | **Loopflow** | done (rename shipped) |
| `lf` / `lfd` | the internals | unchanged framing |

Note `direction` as a config field is already gone (area × flow today); the
**Goal / Loop Prompt** is its conceptual successor — the prompt that steers a
Loop rather than a perspective fragment injected into a prompt.

## Resolved — Step is the internal atom; Flow is the goal-loop's grain

- **Step = internal atom** — the smallest unit `lf` executes (one prompt, one
  agent action). Below the product line, inside the engine.
- **Flow = atomic-ish for a goal loop** — the grain at which an agent running a
  Loop dispatches work: it hands a whole Flow to a Worker and doesn't crack it
  open into Steps. Not a claim that Flow is the *smallest* thing Loopflow
  exposes — the claim is about altitude. A goal-loop agent reasons in Flows; it
  never reasons in Steps.

So the boundary isn't a hard primitive line, it's a *grain* line: above it, a
Loop thinks in Flows; below it, `lf` runs Steps. Worker→Flow is where work
crosses from "what the goal loop dispatches" into "what the engine executes."

## Tension 2 — "Loop = Goal" collapses three Wave fields

A Wave today is `area × flow` plus a `mode` (manual/loop) plus a goal. "Loop"
as a single noun absorbs the loop-ness, the goal, and the work. Two ways to land
it — **this is the open fork:**

- **(A) Loop replaces Wave.** Loop is the config *and* the runtime noun; `mode`
  becomes an attribute (`loop` = always-on, `manual` = runs once). A one-shot
  `lf code` is then "a Flow run," not a Loop. Cleanest naming; biggest rename.
- **(B) Loop is the live instance; Wave stays the config.** A Wave is the
  declared `area × flow × goal`; a Loop is a Wave that's currently running and
  steerable. Keeps `wave/<name>/` on disk; adds Loop as a runtime word only.

Recommendation leans **(A)** — the sketch frames a Loop as the thing itself, not
a runtime view of a config. But (B) is much cheaper and non-breaking. Jack's call.

## Open question — where does the chord/garden layer sit?

Today a root wave gardens member waves (a wave watching waves). In Loop terms:

- Two layers: **Loop → Workers → Flows** (one Loop, many Workers).
- Or three: a **conductor Loop → member Loops → Workers → Flows** (the chord).

Is "the thing a Loop spawns" always a Worker, or can a Loop spawn other Loops?
The goals chord implies the latter. Worth pinning before the noun hardens.

## If adopted: blast radius (separate migration, not this PR)

A real rename, wide:

- `loopflow.api` (`create_wave` → `create_loop`?), `lfq` wave commands
- `wave/<name>/` directory convention + frontmatter
- Concerto UI ("waves" throughout), DTOs (wire types mirrored 3 ways)
- README, config keys, docs, goldens

Big enough to be its own wave. The Loopflow product rename (shipped) is the first
step of this larger reframe; the Wave→Loop rename is the rest of it.
