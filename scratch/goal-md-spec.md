# GOAL.md — the wave charter (P1 design spec)

The canonical design for `GOAL.md`, the durable charter that directs a persistent
looping agent (a "wave"). Companion: `scratch/goal-md-research.md` (prior-art
grounding — OKR / Scrum / AoR / agent-constitutions). This file is the decision;
that one is the evidence.

Sequence: **P1** (this — anatomy + the diagnostic) → **A** (retrofit our waves)
→ **B** (onboarding + maintenance UX). They're one ladder, not three projects
(see the end).

---

## THE TASK — read this first, it's the point

A prior run got the *structure* right and the *substance* wrong: it applied these
section headers to the **old content** — kept vague objectives, *relabeled*
output-metrics as "Key Results," and kept a dark-room metric ("≥20 unattended
iterations") this spec says to kill. **That is the exact failure to avoid.** Your
job is a **substantive rewrite of each charter, not a re-heading of what's there.**
Moving old bullets under new titles is a failure, not a retrofit.

**Priority (from Jack): spend the run on the charters, not the code.** The primary
deliverable is **genuinely good, reframed `GOAL.md` files** for every wave. The
supporting code (dropping `primary_flow`, migration, DTO mirror) is **secondary —
rough is fine, Jack will do a production pass later.** Do NOT burn the run
polishing migrations / DTOs / tests / build-cleanliness. Take a big hack at making
the *charters* excellent. (Leave the tree free of conflict markers, but don't
gold-plate.)

**The done bar for every charter:** read it back and ask it the honest question —
*could a mind reading this name its single most impactful next move, whole-body
yes?* If not, you're not done. If moving old content under new headers wouldn't
get you there (it won't), you haven't started.

**The reframe recipe — apply to EVERY wave** (`goals`, `architecture`, `concerto`,
`meta`, `systems`; read each wave's current `GOAL.md` *and* its `MEMORY.md` for
what it actually does before you rewrite):
- **Objective — REWRITE it.** Do not keep the original sentence. Give each wave a
  *distinct, sharp* objective + vibe you could tell from any other wave's — its
  real judgment prior. Generic-that-could-be-any-wave = failure.
- **Key Results — REFRAME to outcomes, never relabel.** Every KR is an outcome
  with a target (§4). **Kill** every output/codename/deliverable metric and every
  dark-room metric (anything like "iterations ≥ N" that rewards busywork — delete
  it, don't relabel it). Stage-appropriate (prototyping, not growth). If a wave's
  true metric is genuinely unclear, write your best outcome-KR and flag it in
  `scratch/questions.md` for Jack — don't paper over it.
- **Cron — ADD it.** Every wave has recurring duties (reconcile the roadmap,
  curate memory…). Declare them; never drop the section.
- **Process — real routing judgment** (task sizing, which-flow-when), not a stub.
- **Guardrails/scope** fold up into the Objective.

**For `wave/goals/GOAL.md`: §5 below IS the target** — use that reframed charter.
For the other four, do the *equivalent* substantive reframe from each wave's own
reality.

---

## 1. The core idea

**GOAL.md is a wave's constitution — inward-facing, and paid every loop.** It is
re-injected into the mind's context *every iteration*, so the governing rule is
not "what would be nice" but **"what must be true and present in every single
loop."** That makes it the *stable, compressed* thing — the constitution, not the
backlog. Anything that changes loop-to-loop (progress, tasks, fresh learnings)
lives in MEMORY.md or Linear, not here.

**Audience separation (load-bearing):**
- **GOAL.md + MEMORY.md → the mind, for itself.** Identity, direction, memory.
- **Roadmap (Linear) → workers + humans, for communication.** The decomposed,
  actionable task list the mind *emits*. The mind reads it to *coordinate the
  workers it delegated to*, never to learn *what it's chasing* (that's here).
  Registers differ: GOAL.md can be identity-rich and vibey (it's talking to
  itself); the roadmap is crisp and task-shaped (it's talking to workers).

**One wave = one objective.** Unlike OKRs (which stack multiple objectives), a
wave has exactly one, and it *is* the mission/vibe. A second objective forks a
**child wave**, it does not add a row.

---

## 2. The anatomy

```yaml
---
crons: []                       # schedule → duty
pm: { linear_project: <id> }
---
```
> Frontmatter is the machine-read config. `primary_flow`/`task_flow` is **deleted**
> — flow routing is now prose in Process (§Process); the mind chooses, guided.

**## Objective** — *one paragraph.* The one thing this wave exists to do, in a
voice you could tell from any other wave's. This is the mind's **judgment prior**:
when the roadmap is ambiguous, decisions flow from here. The vibe is *functional*,
not decoration — for an LLM re-reading it every loop, it's the entire personality
and taste that makes executive calls coherent. One objective only.

**## Measures** — split by lifecycle, because "Key Result" jams three different
things together:
- **Key Results** *(complete-this — retire when hit, checked for* progress*)*.
  Crib the OKR discipline hard (§4).
- **Quality** *(hold-this — standards checked every loop for* compliance*, never
  "done")*.
- **Bounds** *(never-exceed — checked for* violation*; the spend/resource cap)*.
- **Done means** — the durable bar a unit of work clears before it counts (Scrum
  DoD).

Frame: **GOAL.md is the wave's control loop written down** — objective = direction,
KRs = targets it converges to and retires, quality = invariants held while moving,
bounds = limits it must not cross.

**## Cron** — *scheduled recurring duties, mirroring the `crons:` frontmatter.*
This is the executable half of "responsibilities." (The other half — standing
*ownership* — folds up into the Objective's scope: what's yours to pick up.)

**## Process** — *free text; the one place nuance lives.* How work actually gets
done here: how to **size** a task (big vs small), **which flow when**, decompose
vs. go direct, when to deviate. Flows are defined in `.lf/flows/`; the flow files
hold the *structure*, Process holds the *selection judgment*. Keep it judgment,
not a novel. (This is the section that replaced `primary_flow`: routing is now
prose the mind reasons over, not a config knob that mechanically fires.)

**Deleted on purpose:** an explicit anti-gaming "north" and a "stop" clause. Both
are subsumed by the diagnostic (§3) and are not per-wave-specific — if they belong
anywhere it's the global operating prompt (LOOPFLOW.md), applied to every wave
once. (Decision: just kill them from the charter.)

---

## 3. The quality bar is a question, not a rubric

A rubric scores **presence, not quality** — an LLM ticks every box and stays
mediocre, and it can't touch the two things that matter most (the vibe and the
Process judgment). Replace it with **one honest question the mind asks its own
charter:**

> **"Do I have what I need to know what work from me would be most impactful?"**
> If it isn't a whole-body *yes* — fix *that* first.

It tests the **function** (can you prioritize?) not the **features** (do you have
the sections?), so it's ungameable — you can't fake a felt readiness the way you
tick a box. And it does the job of the deleted sections:
- can't-name-the-impactful-work → objective unclear, or chasing a number over
  intent *(the anti-gaming "north")*
- nothing-is-impactful-anymore → you're done, or you'd be busy-working the dark
  room *(the "stop" / termination discipline)*

**Failure modes are empirical, not theoretical.** Don't ship a canned list of 15;
**discover them by auditing our real waves** (that's phase A) — and seeing real
failures is what *calibrates* the honest yes against an LLM rationalizing a false
one.

**"What good looks like" is examples, not a rubric.** The standard is a **gold set
of excellent, deliberately diverse GOAL.md exemplars** (code / research / PM
shapes — good charters look *different* by wave-type), plus the real failure
examples from our own waves. Phase A *produces* the gold set: our best retrofitted
waves become the exemplars B teaches from.

---

## 4. Key Results — crib the OKR discipline

(Renamed from "Milestones" — "Key Results" *forces* the discipline the label
implies.) What makes a KR good:
- **Outcome, not output.** The one rule. Measure the *result you're after*, never
  the *work you did*. "Shipped the goal primitive" is a to-do; "N apps built from a
  GOAL.md with zero authored steps" is a result. **Codename deliverables are the
  tell — kill them.**
- **Quantified with a target.** Baseline→target or reach-N. If you can't score it
  0→1, it isn't a KR.
- **Verifiable, jargon-free.** Someone outside the wave can tell if you hit it.
- **~3–5, genuinely different** measures of the one objective — not ten, not a
  task list. (Tasks ladder *up* to KRs; they live in Linear.)
- **Ambitious.** ~70% = a win; always hitting 100% means you sandbagged.
- **Stage-appropriate.** Prototyping KRs measure *does it work for us*; growth KRs
  measure *do others adopt it*; scale KRs measure *does it hold up*. A
  stage-mismatched KR (a growth metric in a prototype) makes the wave optimize the
  wrong horizon — its own failure mode.

---

## 5. Worked example — the goals wave's own charter (reframed)

```yaml
---
crons:
  - "daily → reconcile Linear against what landed: retire done, surface drift"
pm: { provider: linear, linear_project: 'fbdd6124-6114-4427-b6ac-5788dead4f87' }
---
```

**## Objective**
You exist to make loopflow's own thesis true: **writing a goal is a way to
compute** — proven not by a one-off build but by waves that run *consistently*,
doing real work for a week straight, across loopflow *and* Cadenza (a codebase
that isn't your own). Every other wave still scripts its steps; you refuse to. You
own the goal primitive (step · flow · **goal**) and the wave-as-durable-loop model
that runs it. Allergic to ceremony and scripting; you don't trust a claim you
can't demo.

**## Measures**
- **Key Results** *(retire when hit)*: **≥ 5 waves running consistently for 1 week
  straight, across both Cadenza and loopflow** · *(leading)* **multiple waves
  running consistently on both codebases** · **starting a wave elicits a
  whole-body-yes GOAL.md from the human** *(= phase B; see below)*.
- **Quality** *(hold)*: waves keep their GOAL.md current — passing the honest
  question over time, not drifting stale · a landing ships real product code,
  never a design-only PR.
- **Bounds** *(never exceed)*: wave spend stays under its cap.
- **Done means**: a landed PR of real product code, roadmap item closed + PR-linked.

**## Cron**
- `daily` → reconcile Linear against what landed; retire done, surface drift.

**## Process**
The live task set is the Linear roadmap — read it each loop; *it's* the backlog,
not this file. Size before you route: a mechanical change goes direct to a worker
in a fresh worktree; anything with unclear scope or cross-cutting blast radius gets
a scratch design doc + a review pass first. Big moves land as their own small,
reviewable PRs, through `lf op land`. If a landing wouldn't ship real product code,
it isn't ready.

**Audit result:** the codename "Milestones" reframed into outcome KRs anchored on
*reliability across two codebases* (not one-shot demo, not premature
external-adoption). This **resolves the priority thrash** the old charter had —
architecture, reference-builds, and meta-projects all now ladder to one metric
("waves run consistently"), so the whole-body-yes has a real answer.

---

## "Running consistently" — the definition that closes the loop

A wave "running consistently" = each loop it does real, impactful work without
wedging or flailing — i.e. **its mind can answer the whole-body-yes every loop.**
So the reliability KR and the self-diagnostic measure the *same thing at two
timescales*. And *aggregated across the fleet*, the whole-body-yes **is** the
goal-lifecycle metric: what fraction of waves currently pass the honest question,
and how fast a new one got there.

---

## The three phases are one ladder

- **P1 (this):** the anatomy + the honest question + the exemplar. Plus the
  migration: delete `primary_flow` (a durable `Wave` field — lfdb column, 3-lang
  `WaveDto`, `WaveConfig`, every `GOAL.md` frontmatter — `wave_repos`-shaped),
  rename Milestones→Key Results, and put the honest-question / elicitation
  discipline in LOOPFLOW.md (universal).
- **A (retrofit):** audit each current wave through "could its mind name its most
  impactful work?" — *discover* the empirical failure modes, and *produce* the
  gold-set exemplars. Real product code, not a design pass.
- **B (onboarding + maintenance):** the UX. **The mark of good is not "the mind
  authored it itself" — it's "the mind got the right stuff *out of the human* to
  orient correctly."** GOAL.md is the human's intent; the mind can't invent the
  objective/KRs/vibe. So B is **elicitation**: the failed whole-body-yes doesn't
  send the mind to *guess*, it sends it to *ask* — interview the human on exactly
  the missing piece, draft, play it back. The mind does the *labor* (drawing out,
  shaping, assembling), the human provides the *direction*.
  - And the elicitation is **opinionated, not stenography** — the mind is the
    **smoothest groove toward ways that are effective, maintainable, and
    scalable.** The human brings the *what*; the mind grooves the *how* using the
    anatomy, the OKR discipline, and the honest question as the channels — so a raw
    intent comes out as an outcome-KR instead of codename babble, measures split by
    kind, priority resolved — invisibly, while still capturing what the human
    actually wants. Good design makes the right way the easy way.
  - **B is itself a KR of the goals wave** (§5). It's the leading indicator of the
    reliability metric: fix the authoring, the consistent-running follows.

**The metric it all ladders to:** waves running consistently for a week across
Cadenza + loopflow. Goal-lifecycle quality (the groove) is the leading indicator;
a wave that can't get to and hold a clear GOAL will flail, not sustain.

---

## Decision log (the why, compressed)

- **One objective per wave** (vs OKR's many) — a second objective is a child wave.
- **Measures split by lifecycle** — KR (retire/progress) · Quality (hold/compliance)
  · Bounds (never-exceed/violation). OKR conflates these; the lifecycles differ, so
  they're checked differently.
- **`primary_flow`/`task_flow` deleted** — routing becomes Process prose the mind
  reasons over; flows/flow-files stay (the repertoire + their structure).
- **Responsibilities → Cron**; standing ownership → the Objective's scope.
- **Rubric → one honest question + exemplars** — tests function not features;
  ungameable; failure modes empirical.
- **"North" and "Stop" killed** from the charter — subsumed by the question; not
  per-wave-universal (belong in LOOPFLOW.md if anywhere).
- **KRs stage-appropriate** — prototyping measures reliability-for-us, not
  external adoption; Cadenza is the right-depth "beyond our own repo" validation
  without needing outsiders.
- **Onboarding = elicitation with an opinionated groove** — get the right stuff out
  of the human, shaped toward effective/maintainable/scalable; not autonomous
  generation.
