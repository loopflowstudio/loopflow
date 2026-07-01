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

**`area`**: deleted (not dormant). Agents self-scope; the maintainer doesn't use
it. Removed as part of M1 — see "Ruthless simplification" below.

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

## Ruthless simplification — collapse to the loop handles (FOLD INTO M1)

> **To the coding agent:** this is part of the current work, not a later PR. As
> you land `goal.md` + `metrics`, also make these cuts so the wave model ends up
> ruthlessly focused on what we do *now*. The old model was `area × flow ×
> direction`, woken by a cold ticker, configured by a repo YAML. The model now:
> **a Wave is a goal agent** — the loop handles (*flow*, *roadmap*, *metrics*)
> plus the *goal* prompt that directs them. Anything that isn't a handle is dead
> imagination. Delete it; don't preserve it.

**Keep — the loop handles + goal:** `goal` (prompt), `metrics`, `primary_flow`
(the flow handle — first-class, stays on the record), `roadmap` (Asana, M2).

**Cut — the old-model scaffolding:**

| Cut | Why |
|---|---|
| `area` (field + store column + DTO + `budgets.area` + prompt "area" section + CLI `--area`) | Agents self-scope; scope dimension is dead. Delete, don't leave dormant. |
| `direction` **wave-level** (`Wave.direction`, `WaveRunSnapshot.direction`, DTO, store) | Superseded by `goal` (2026-06-19). **KEEP** the step/flow `.lf/directions/` doc system — that's a different concept; don't touch it. Clean cut: see inventory below. |
| ~~`WaveMode` / `Manual`~~ **DO NOT CUT IN M1** | Load-bearing: `list_loopable_waves` filters `mode='loop'`; Manual exempts cron + inline/goal waves from the ticker and gates branch advancement. It's an old-activation-model artifact — dies with the activation redesign (M2+), not now. `mode` lives on `Wave` (runtime), not `WaveConfig`. |
| `WaveConfig`'s all-`Optional` + `Default` YAML-mirror shape; `read_wave_config` YAML path | Replaced by the parsed `goal.md`. |
| `crons`/`triggers` **repo seed path** | lfd-only runtime state now (see DECISIONS). The *feature* stays; the repo origin goes. |
| `step_agents`, `serialized` | YAML knobs, die with the YAML. |
| `wave_*_is_required` / serde-enforcement tests | Test serde, not the product. A non-`Option` field is already required by the type. One round-trip fixture proves the wire shape; delete the rest. |

**`Wave` vs `WaveConfig` — collapse the duplication (do this here):**
Today both list `goal`/`primary_flow`/`metrics`. Give each one identity:
- **`WaveConfig` = the parsed `goal.md`** — the authored contract only: `goal`
  (prompt), `metrics`, `primary_flow`, `agent`, `roadmap`, `workers`. Required
  fields *required* (no all-`Optional`, no `Default` derive).
- **`Wave` = a running instance** — `{ id, name, repo, config: WaveConfig, status,
  iteration, worktree, … }`. Runtime state wrapping the config; **no duplicated
  config fields.**

The partial-update DTOs (`WaveConfigUpdate`, `RunOverrides`) keep `Option` — there
`None` legitimately means "don't change this field." That's the *only* place
Optionality survives.

*(Precise file/line deletion inventories for `area` and `direction`+`mode` are
being compiled and will be appended below — treat them as authoritative for blast
radius when they land.)*

## Milestones

**M1 — `goal.md` is the authored surface + the ruthless collapse above.**
1. `goal.md` format + parser → `WaveConfig` (frontmatter → fields, body → prompt).
2. Resolver adds the `wave/<name>/goal.md` first lookup.
3. `Wave` becomes `WaveConfig` + runtime state; wave creation reads `goal.md` into
   `WaveConfig`. Retire the YAML (`read_wave_config`) and its crons/triggers seed.
4. Execute the cut table: delete the **context-scoping** use of `area`, wave-level
   `direction`, `step_agents`, `serialized`, and the serde-enforcement tests.
   **Not** `mode`/Manual (M2+ activation redesign). Mind the `area` behavioral
   tendrils below — this is not a blind delete.
5. `metrics: Vec<String>` reaches `render_goal`'s context so the loop sees its
   criteria. Migrate `wave/goals/goals.yaml` → `wave/goals/goal.md`.

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

- Deleting `primary_flow` — it's first-class (the flow handle), stays.
- Redesigning activation (persistent 24/7 loop vs. triggered) — M2+.
- Cutting `mode`/`WaveMode::Manual` — deferred to the activation redesign (M2+).

## Deletion inventories (authoritative blast radius)

### wave-level `direction` — CLEAN CUT

Plumbing (delete): `Wave.direction` (`types/wave.rs:240`), `WaveRunSnapshot.direction`
(`:347`), `WaveConfig.direction` (`wave_config.rs:58`), `WaveDto.direction`
(`dto.rs:90`), `WaveRunDto.direction` (`dto.rs:124`), migration `001_initial.sql:6,71`
(add a drop migration), `store/rows.rs:104,180`, `store/catalog.rs:171-226` (queries),
`store/sqlite.rs:209,1214,1250`, `store/postgres.rs:219,1301,1341`.

Behavioral (all just pipe `wave.direction` into `-d` CLI flags): `waves.rs:213-215,626-628`
(payload → wave), `helpers.rs:62,148` (snapshot capture), **the boundary** →
`helpers.rs:369-408` `build_lf_step_command()` + call site `executor/wave/mod.rs:294`.
Cut = pass `&[]` (or drop the param). **Do not touch** `engine/prompt.rs:662-668`,
`engine/flow.rs`, `fork.rs::merge_directions`, or step frontmatter `directions:` —
that's the `.lf/directions/` doc system and it stays whole.

### `area` — CUT THE CONTEXT USE; MIND THREE TENDRILS

~450 plumbing sites (fields/serde/migrations/DTOs/fixtures across Rust + `models.py` +
`Wave.swift` + `001_initial.sql:7,72`). The **context-scoping** behavior is the dead
one — delete it: `engine/prompt.rs:188-189` (area auto-enables `DocumentSource::Area`),
`:781-810` (`resolve_area`/`gather_area_docs`), `:545-553` (budget trim), `:1716-1719`
(render), `config.rs:232` (`budgets.area`), `--area` CLI (`bin/lf.rs:13,34-35`,
`run.rs:129-132`), `lf-prompt.rs:48,66`.

**Three tendrils that are NOT context-scoping — handle explicitly:**
1. **Repo-trigger scoping → moves onto the trigger (DECIDED: B).** A repo trigger
   currently fires only if a changed path is in `wave.area` (`triggers/watch.rs:129-137,
   182-187`). Scoping stays, but as the **trigger's own** concern: add a path-pattern
   field to the trigger definition (`TriggerDef` in `wave_config.rs`, the wire
   `Trigger`/`TriggerDto`, and the store), and rewrite `paths_match_areas` to match
   against the trigger's patterns instead of `wave.area()`. Empty patterns = match all.
   This is the *right* home for scoping (a reflex knows what it watches); `wave.area`
   was borrowing the job.
2. **Summary gating** — `executor/wave/summary.rs:21-65` + `engine/git.rs:830`
   (`hash_areas`): area gates whether/what a wave summarizes. **Lean: drop the
   area-gated wave summary** — it existed to summarize a *scoped* area; with no area
   it has no subject. If global summaries are wanted later, that's a separate feature.
   (Small call; flag if you disagree.)
3. **Release scoping is a DIFFERENT `area`** — `ops/release.rs:756-825,1020` uses
   **`config.area`** (release-target manifest/PR scope), not `wave.area`. **Do not
   delete it** with the wave cut. `engine/config.rs:267` `Config.area` stays.
