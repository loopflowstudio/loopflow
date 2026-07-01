# Next unit — `goal.md` as the one authored wave surface

Follows the goal primitive (`scratch/jack-heart.wave-looping-agents.md`, landed).
Vision: `wave/goals/README.md`. Decisions + why: `release/unreleased/DECISIONS.md`
(2026-06-30).

## The model (settled)

**A Wave *is* a goal agent** — not a container that holds a goal loop, the looping
agent itself. It is authored by one file, `wave/<name>/goal.md`.

**Repo = authored intent. lfd = runtime state.** The old `wave/<name>/<name>.yaml`
fused the two (it seeded lfd wiring like crons from the repo). Splitting them is
the point of this unit, not an incidental cleanup.

| Layer | Repo (authored, portable) | lfd (runtime, live) |
|---|---|---|
| Wave = goal agent | `wave/<name>/goal.md` | status, iteration, activation |
| Work it dispatches | `.lf/steps`, `.lf/flows`, `.lf/goals` | dispatched runs |
| Reflexes | `.lf/crons`, `.lf/triggers` *(new, M3)* | which fire on which wave, last-fired |

## `wave/<name>/goal.md`

Frontmatter = intent. Body = the loop prompt.

```markdown
---
roadmap: asana://1207xxxxxxxx     # the Asana project this wave steers by
metrics:                          # prose criteria the loop re-judges each pass
  - all wave/goals items shipped
  - cargo test green; clippy clean
  - Concerto can author a goal and spawn an agent from it
agent: claude:opus
workers: 3                        # fan-out cap (max parallel subagents)
primary_flow: build               # default flow dispatched per task
---

Run one loop iteration against this wave's roadmap.

Read the roadmap, pick the next useful move, dispatch the appropriate flow, and
leave the wave closer to done. If no safe move remains, record the blocker
instead of inventing work.
```

**Resolver** (extends the landed lookup — one new first entry):
`wave/<name>/goal.md` → `.lf/goals/<name>.md` → `~/.lf/goals/` → builtin. The
landed `wave.goal: String` stays as the resolution key; a local `goal.md`
overrides it with body + frontmatter. Builtins (`ship-roadmap`) become templates
you copy, like steps.

**Metrics**: a frontmatter list of prose criteria the loop reads and self-judges.
No measurement engine — the agent evaluates them ("I'm not micromanaging").
Promote individual metrics to machine-checked exits later, only where a hard stop
is needed.

**`area`**: deprecated. Agents self-scope; the maintainer doesn't use it. Left
dormant (~300 refs), stripped in its own PR. Nothing new builds on it.

## Crons / triggers — not in `goal.md`

A cron/trigger is a **flow-level reflex** ("on signal X, fire flow Y") — a lower
primitive than the goal agent, no agent reasoning in the loop. Two origins:

- **Authored** — a human records it in `.lf/crons`/`.lf/triggers` to make it
  portable/installable, like `.lf/steps`. Deliberate, version-controlled.
- **Wave-spawned** — the goal agent creates one while operating. lfd-only state,
  **never written to disk** (no auto-dump of every runtime reflex).

Neither lives in `goal.md`. This is M3, not M1.

## The dispatch contract (landed this session)

In `LOOPFLOW_OPERATING_PROMPT` (`rust/loopflow/src/engine/flow.rs`). Capture, then
dispatch — the agent never solves substantial work inline: writes an Asana task,
launches a flow against it (`lf <flow>: <task>`) as a steerable session,
re-measures. Fan-out (parallel subagents, one flow per task) gated on budget +
well-scoped tasks.

## Milestones

**M1 — `goal.md` is the authored wave surface.** ~800 LOC, no Asana/Concerto dep.
1. `goal.md` format + parser (frontmatter → config, body → prompt).
2. Resolver adds the `wave/<name>/goal.md` first lookup.
3. Wave creation reads **intent frontmatter** into the `Wave` record — replacing
   the YAML path in `read_wave_config`. Only intent fields flow from the repo.
4. Retire `wave/<name>/<name>.yaml` **including its crons/triggers seed path**
   (those become lfd-only; no repo origin).
5. `metrics: Vec<String>` lands on the wave; `render_goal` context gains it so the
   loop sees its criteria. Migrate `wave/goals/goals.yaml` → `wave/goals/goal.md`.

**M2 — Asana `roadmap:` read + write-back.** The live heartbeat: loop steers by a
real backlog, goal agent writes tasks back (`wave/goals/2-asana-roadmap.md`).
Asana client + auth is the real surface here — why it's its own milestone.

**M3 — `.lf/crons` + `.lf/triggers` as portable reflex definitions.**

**Later — Concerto author/spawn/steer looping sessions; budget primitive.**

## Open (do not block M1)

- `goal.md` machine-checked metrics (M2+, only where a hard exit is needed).
- Budget primitive shape (`wave/goals/2-wave-budget.md`, `scratch/questions.md`).
- Persistence backend a1/a2 (`wave/goals/4-*`).

## Out of scope

- Removing `area` plumbing (dormant now; own PR).
- Deleting `primary_flow`.
- Redesigning activation (persistent 24/7 loop vs. triggered) — M2+.
