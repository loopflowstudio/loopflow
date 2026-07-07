# Flowloops: the OKR-tiered agent model (wave / project / task)

The consolidated design from the 2026-07-06/07 sessions. Supersedes
`scratch/worker.md` and `scratch/minds.md` (absorbed here) and resolves against
`worker-build-brief.md` and the recovered `wave/DATAMODEL.md` (#818). This is the
single doc to build from.

**Vocabulary:** a **flowloop** is *a flow that is looped* — the flow
`clarify → pursue_goal → mutate`, repeated until its terminate arm fires. Every
agentic long-running thing in loopflow is a flowloop; "mind" is retired as a
noun (code renames in §4.1).

---

## 1. The model in one screen

Everything agentic is the **same flowloop runtime**. Flowloops differ on exactly
**two axes**:

1. **Ownership** — which concrete artifact/PM unit it owns and clarifies.
2. **Stop-condition** — a **deterministic, non-agent oracle** that decides the halt.

The tiers form an OKR decomposition:

| Flowloop | Owns (and clarifies) | Stops when | Lifecycle |
|---|---|---|---|
| **wave** | the **Objective** — mission · vision · vibe (`GOAL.md`). *Not* the KRs. | **never** — told never to stop. Mutates: re-invokes ~every 24h, may split or be stopped over days–months | persistent identity (GOAL + MEMORY + vibe) |
| **project** | a **set of 2–10 KRs** pursuing the objective — a sequence of PRs pursuing one idea in full | **all its KRs are done.** A *self-renewing* KR respawns on completion ⇒ such a project runs forever; a milestone-only project may finish in a day | as long as its KRs |
| **task** *(formerly "worker")* | a **design doc → one small PR** advancing a KR | its **PR is done** — landed on `main` (later: verified in prod) | ephemeral (minutes–hours), own worktree |
| **exec** | nothing — one `lf` invocation run in a flowloop's worktree | returns | not a flowloop, no loop, no branch |

Key consequences:

- **KRs move out of `GOAL.md`.** The wave owns only the objective; the measurable
  KRs live at project level. The measure-kinds from the GOAL.md research
  (milestone / quality / constraint) describe a **project's KRs** now: milestone
  KRs retire, quality/recurring KRs self-renew (the "keeps going automatically"
  knob), constraints bound.
- **No fractal branch-stacking.** A project coordinates its tasks through the PM
  tool (KRs → tasks), not stacked branches. **Every task lands a small,
  independent PR to `main`.** Nothing accumulates; the gigantic-top-merge problem
  never exists. (The DATAMODEL self-draining cascade is recorded in §7 as a
  rejected-for-default alternative.)
- **Self-enforcing completeness.** A project cannot finish by hiding work:
  discovered debt/followup is filed as a task (or KR), which keeps the project
  non-empty until the work is genuinely done.

## 2. Termination: deterministic oracle, agentic everything-else

**The heart of the design.** The flowloop chooses *moves*; it never decides it is
*done*. The halt is `if (oracle) stop` where the oracle is non-agent ground truth:

- task → `gh pr view` says MERGED (later: + a prod-verification check)
- project → the KR set reads all-done (KRs are measurable by definition)
- wave → no oracle; the loop is the point

The agent can't fake completion or dark-room its way out — GitHub/Linear/prod
decide, not the model's self-report. The terminator is a **composable predicate**:
swap in `spend ≥ $N` for a budget-bounded explorer; AND them for "finish, but stop
at $N either way."

**Enforcement (v1, revisitable):** the termination clause lives **in the agent's
seed** ("poll the oracle; stop only when terminal") and each phase run carries
**`-b` budget + wall-clock timeout as the harness-enforced backstop the agent
cannot override**. Two deterministic levels: soft self-halt (cheap), hard floor
(non-overridable). A purist external-terminator is deferred unless self-halt
proves unreliable.

## 3. The flowloop body — one shape for every tier

```
loop {
    clarify()       // make the owned artifact clear enough to compute from.
                    // Can be: a noop (already clear) | an INTERACTIVE chat
                    // exchange with the user (review-wave-goals / review-krs /
                    // review-design) | background work (research, rewrite).
    pursue_goal()   // one pass of work advancing the owned unit
    mutate()        // the lifecycle step:
                    //   if should_terminate() — the deterministic oracle (§2):
                    //     task → PR merged; project → all KRs done; wave → never
                    //     → terminate
                    //   else consider:
                    //     - update GOAL.md from progress learnings
                    //     - launch sub-waves
                    //     - renew a self-renewing KR
                    //     - reset (wave's ~24h re-invocation) | split | continue
}
```

The tiers are instantiations of this one body: what differs is the **artifact
`clarify()` targets** (GOAL.md / KR set / design doc), the **work
`pursue_goal()` does**, and **which `mutate()` arms are live**: the oracle behind
`should_terminate()` (task: PR merged; project: KRs done; wave: constant false),
then the living arms — a task just continues; a project adds renew; a wave
updates its own GOAL.md from progress learnings, launches sub-waves, resets, or
splits. Clarify-first encodes the standing rule: a flowloop whose artifact is too
vague to compute from fixes the artifact before inventing downstream work — and
escalating that to a human is just `clarify()` choosing the interactive branch.

**Implementation & surfacing (decided):**
- **A flowloop is a flow that is looped** — the flow (loopflow's existing
  primitive: a sequence of steps/skills) `clarify → pursue_goal → mutate`,
  repeated until `mutate()`'s terminate arm fires. Each phase is a skill executed
  **with `-b`** (budget/time-bounded, headless). The one new primitive is
  *loop-a-flow-until-terminator* — everything else is existing flow machinery.
  Nuance vs §7: what stays rejected is a Rust *move policy*; the flow
  *sequencing* is structured, the judgment inside each phase is fully agentic.
- **The tier is declared at spawn, and it binds the phases to TARGETED skills.**
  A flowloop knows it is a wave/project/task in advance — the shape is shared,
  but each tier runs its own skill per phase (a 3×3 matrix):

  | | `clarify` | `pursue_goal` | `mutate` |
  |---|---|---|---|
  | **wave** | `wave_clarify` — GOAL.md | `wave_pursue` — launch projects/tasks; hot execs directly | `wave_mutate` — update GOAL.md, sub-waves, reset, split; never terminates |
  | **project** | `project_clarify` — the KR set | `project_pursue` — decompose KRs → spawn task flowloops; file debt | `project_mutate` — retire/renew KRs; terminate when all done |
  | **task** | `task_clarify` — the design doc | `task_pursue` — work the PR: code, CI, review | `task_mutate` — terminate when PR merged |

  Generic prompts ("clarify your artifact, whatever it is") are exactly the
  vagueness this kills: each skill states its tier's artifact, move menu, and
  oracle concretely. Tier-specific behavior lives in the **skill texts**, not in
  runtime branching — evolving what a project flowloop does = editing
  `project_*.md`, no code change.
- The phase runs are **plumbing — never surfaced in the Loopflow product.** No
  per-phase session UI.
- **Chat is the single interface to every flowloop** — wave, project, and task
  alike. Humans (and parent flowloops) speak to a flowloop on its channel;
  `clarify()`'s interactive branch is a chat exchange, not a separate surface.
- **Only execs surface as tmux sessions you can take direct control of.** The
  surfacing split is exactly the flowloop/exec split: **flowloops you talk to
  (chat); execs you can grab the wheel of (tmux attach).** Direct control lives
  at the layer with no judgment loop to disrupt.

## 3b. What each tier does (behavioral contracts)

**wave** — the eternal gardener. Re-reads `GOAL.md`, watches its projects,
curates MEMORY. Two modes of action:
- **Standard progress: delegate.** Launches **project and/or task flowloops** to
  pursue the goals — the wave orchestrates, it does not grind.
- **Hot problems: act directly.** For urgent, immediate issues (main is red, a
  release is wedged, a fire in its domain) the wave may run **execs itself** —
  no ceremony of spawning a flowloop when the fix is one command away. The
  boundary: execs for *hot/now*, flowloops for *planned/tracked*. If a "hot fix"
  grows into real work, file it and spawn the flowloop.

**project** — drains its KR set. Decomposes KRs into tasks (design docs), spawns
task flowloops, files discovered debt as new tasks, retires milestone KRs,
respawns self-renewing ones. Reports to the wave by outcomes.

**task** — the v1 build (§5). Takes one task, works in an ephemeral worktree:
Work → PR (to `main`) → Watch CI → Fix → Land → die. Moves are agentic; mechanical
steps are cheap ops; judgment steps (Work/Fix) dispatch execs / write code.

Shared contracts (all non-wave flowloops):
- **Pen-transparent.** No durable channel curation or MEMORY of their own;
  children's chat/memory bubble up to the owning wave (memory already auto-routes
  to the family head — `lf/commands/memory.rs`). Their state **is their PM unit +
  branch**.
- **Steer by input + lifecycle, not conversation.** The parent steers via
  dispatch (the unit + `-b` + merge authority), interrupt/kill, and
  amend-and-re-dispatch. **The branch is the checkpoint** — re-dispatch is cheap
  because nothing lives in the flowloop's head. Chat *to* a flowloop is input it
  folds at phase boundaries, not mid-phase puppeteering (waves-outward C6).
- **HITL = chat.** Every flowloop is spoken to on its channel; the named review
  checkpoints (`review-wave-goals` / `review-wave-metrics`, `review-krs`,
  `review-design`) are chat exchanges where the flowloop presents its artifact
  and folds in explicit feedback. **Direct control exists only at the exec
  layer** — execs surface as tmux sessions a human can attach to and drive.
- **Escalation** = `lf chat --parent` + nonzero exit; the parent decides
  retry / reassign / drop.
- **Merge authority:** `Submit` default (human clicks merge); `Land` opt-in per
  dispatch.

## 4. Decisions locked

1. **Flowloop is the noun; "mind" is retired — everywhere in the code.**
   Renames land in run 2 (see `scratch/flowloop-run2.md` §3):
   `wave/mind.rs` → `flowloop/wave.rs`, `run_mind` → `run_flowloop`,
   `MindEnd` → `FlowloopEnd`, `MindConfig` → `FlowloopConfig`, plus the
   comment/doc sweep; `MindState` (wire DTO) may split into a follow-up.
   The judgment steps are agentic; only the halt is deterministic.
2. Deterministic terminator per §2; in-agent clause + `-b` backstop.
3. Tiers = ownership × stop-condition (§1). "Worker" is renamed **task**.
4. Task PRs target **`main`**; stacking is opt-in for genuinely dependent work
   only (#836 re-parents on land).
5. Pen-transparency + bubbling per §3b (topology: attach to the wave's one
   listener; no per-flowloop listener server).
6. Lineage in metadata (`parent_run_id`…), stamps from dispatch; wave home
   permanent; ephemeral worktrees self-prune (#818 webhook).
7. Wave sees **outcomes** (fold_workers + PR links), not turn-by-turn activity.
8. Oracle signal is **polled** (gh/Linear), no webhook dependency.
9. **A flowloop = the flow `clarify → pursue_goal → mutate`, looped until
   terminate.** Phases are `-b` skills, invisible in the product; the flowloop is
   the one new primitive. Surfacing splits by kind: **flowloops: chat-only.
   Execs: tmux sessions you can take direct control of.**
10. **Tier declared at spawn → targeted skills.** The 3×3 matrix
    (`wave_clarify` … `task_mutate`, §3): shared shape, per-tier skill texts.
    Tier behavior evolves by editing skills, not runtime branching.

## 5. v1 scope — the task flowloop, end to end

**v1 = `lf task <linear-task>` (working name) runs the whole loop unattended:**
opens a small PR on `main`, drives it green (fixing at least one red CI), lands
via `Submit`, terminates on the `gh` oracle, worktree self-prunes. Bounded:
attempt/budget caps, **waiting ≠ thrashing**, non-convergence fingerprint,
Blocked-exit escalates via `lf chat --parent`.

Build order: **v1a** happy path (shipped: `lf task` + `flowloop/task.rs`) →
**run 2** the runtime itself — tier-generic `flowloop/` module, the wave
converted to run as a flowloop, the project tier built (unwired), the
mind→flowloop rename sweep; see `scratch/flowloop-run2.md` → **v1b** fix loop
(CI/rebase/review) → **v1c** unattended hardening → **v1d** surfacing (chat to
the task flowloop; its execs visible as attachable tmux sessions; phase runs
invisible).

**v2+:** project tier wiring (the wave spawns projects; `lf project` or
equivalent); chat review checkpoints; prod-verification oracle;
Exec=Run-minus-worktree substrate; opt-in stacking; Concerto surfacing.

Acceptance bar for v1: point it at a real Linear task, walk away, come back to a
merged reviewable-size PR and no running process. Kill switch never needed.

## 6. Hardening backlog (prior-art, from the build brief)

v1c subset: attempt/budget caps + max-restarts-in-window (OTP ladder);
non-convergence fingerprint `(tool,args,result)`; flaky-vs-real gate on Fix
(reproduce-then-fail, not blind retry); idempotency keys on Work/Fix/Land.
v2 (with the project flowloop): subtree-wide depth/spawn caps (the LangGraph
subgraph footgun), spawn-storm cap + cost accounting, class-based escalation,
merge-queue batching/backoff, poison-PR dead-letter.

## 7. Rejected / deferred alternatives (recorded, don't re-litigate)

- **b1 / DATAMODEL-(b): deterministic Rust move-policy** (the scrapped
  `worker.rs`) — judgment belongs to the agent; only the halt is deterministic.
- **Fractal fork-to-`parent()` + self-draining cascade + `Sealing`** — the stack's
  root PR contains its whole subtree (gigantic top merges). Deferred opt-in;
  the project tier makes it unnecessary as a default.
- **"Mind" as the noun** (and the interim `run_loop` idea) — settled on
  **flowloop**: a flow that is looped.
- **Raw-session attach as the task's HITL surface** — explored (the
  wave-chat/worker-session inversion), superseded: **chat is the interface to
  every flowloop; only execs are attachable tmux sessions.**
- **Live conversational down-steer of a running flowloop** — M4 at best; chat is
  folded at phase boundaries.

## 8. Open questions

1. **KR representation in Linear** — what concretely is "a project's KR set" in
   Linear's schema, and what makes a KR machine-readably "done"? (Research R1.)
2. **Merge authority** — when does `Land` (auto-merge) become the default? The
   trust gate for full autonomy.
3. **Prod-verification oracle** — the deterministic check that upgrades
   "complete" from merged to verified.
4. **Command surface** — `lf task` / `lf project` verbs vs a `--tier` flag on
   dispatch; naming vs the existing `SessionUse::Worker`.

## 9. Technical research to start (ordered)

- **R1 — Linear as the KR substrate.** Can Linear model project-with-KRs
  natively (projects, milestones, labels)? What's queryable enough to be an
  oracle? Fallback: KRs as a typed doc in the repo mirrored to Linear tasks.
- **R2 — the flowloop runner vs `run_mind`.** The flowloop is *a flow that is
  looped* (three `-b` skill runs per pass), not a long conversational session —
  so: does the flowloop runner replace `run_mind` as the execution model (with
  the listener kept for chat), or does `run_mind` host the flowloop as its turn
  structure? Read `wave/mind.rs` + the flow executor; find the smaller change.
  Also: where the terminate arm hooks in, and how chat input reaches a flowloop
  at phase boundaries. (Code renames per §4.1 ride this work.)
- **R3 — budget backstop.** Verify `-b`/budget + wall-clock enforcement in the
  current `lf` run surface; what force-end looks like mid-phase.
- **R4 — oracle plumbing.** `gh pr view` polling through the exec door; verb
  allowlist for a task flowloop (door-review rec 2); Linear read for KR state.
- **R5 — chat review checkpoints.** How a flowloop runs `review-design` etc. as
  a chat exchange on its channel — request feedback, wait bounded, fold the
  answer — with the existing `lf chat`/listener machinery.
- **R6 — the skill matrix.** Draft the nine tier-phase skills
  (`wave_clarify` … `task_mutate`) as builtin steps; decide home
  (`engine/builtins/`) and how the tier binding selects them at spawn.

## 10. Source map

- Loop runtime: `wave/mind.rs` (`run_mind`, `MindEnd`, biased select, heartbeat),
  `wave/resident.rs`, `wave/supervisor.rs` — to be renamed per §4.1.
- Identity/worktrees: `engine/identity.rs` (`WaveId`, #818); self-prune webhook
  (#818); reparent-on-land `ops/rebase.rs` (#836).
- Dispatch: `lfd/executor/helpers.rs`; session role `SessionUse::Worker`.
- Exec door: `POST /v0/exec` (#825) + `scratch/door-design-review.md` (research
  worktree).
- Memory family-head routing: `lf/commands/memory.rs`.
- KR/measure-kinds research: `scratch/goal-md-research.md` (research worktree).
- Prior handoff: `scratch/worker-build-brief.md` (this worktree);
  DATAMODEL recovered via `git show 098bbcaf:rust/loopflow/src/wave/DATAMODEL.md`.
- Roadmap: goals wave, item `18603b7f-642e-4588-9622-23f8a15fd4f4` (retitle from
  "worker" to task flowloop when next touched).
