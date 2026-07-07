# Worker build brief — everything the Worker branch needs

Consolidated 2026-07-07 from the landed #818 work + two design passes + prior-art
research. This is the single handoff: decisions, resolved forks, and the
hardening the design hasn't absorbed yet.

## Where the Worker actually is

- **Design specced in main:** `rust/loopflow/src/wave/DATAMODEL.md` (Wave/Worker/Exec,
  state machine, self-draining cascade, 4 open decisions).
- **Identity substrate shipped (#818):** `engine/identity.rs` — `WaveId` with
  `parent()`, `is_worker()`, `depth()`, `chain_str()`, two decoupled projections
  (flat `dir_component()`, `/`-scoped `branch()`). Dispatch/mind/supervisor all
  exist and are reusable (`lfd/executor/helpers.rs`, `wave/resident.rs`,
  `wave/mind.rs`, `wave/supervisor.rs`).
- **Policy already coded:** `rust/loopflow/src/worker.rs` on branch
  `jack-heart.worker-lifecycle.20260706_1730` — `WorkerLoop::next_decision(&WorkerObservation)
  -> WorkerDecision`, `WorkerMove`, `MergeAuthority` (default `Submit`). Pure,
  side-effect-free, tested. The *runner* around it is deferred (its `questions.md`).
- **Research branch:** `goal-md-research.worktreeworkers` — GOAL.md + Door-auth
  research only, no Worker code. Load-bearing claim: *"a wave has exactly one
  objective; a second objective forks into a child wave."*

Recover the two scratch design docs (wiped from main on land):
`git show b1827228:scratch/worktree-redesign.md`,
`git show 7c67113a:scratch/worker-lifecycle.md`.

## Decision ledger (settled)

- **Wave/Worker/Exec.** Wave = persistent mind, arbitrary goal. Worker = ephemeral
  mind, goal "land one PR to `parent()`, then die." Exec = one `lf` invocation, no
  mind, no branch, runs in a mind's worktree. A chain segment ⇔ a mind ⇔ one
  PR-to-parent; Execs get no segment.
- **Dispatch = fork-and-target `parent()`.** One rule all tiers; `parent()==None ⇒
  main`. Collapses `Placement::Fresh`(→main) and `Stack`(→parent).
- **Stamps come from dispatch, not `wt create`.** Waves stamp-free; workers carry a
  trailing `.ts`, re-minted per descent (`a.b.c.<new ts>`, never `a.b.ts.c`).
- **Lineage lives in metadata, not the name.** DAG on `Run.parent_run_id`/
  `stack_group_id`/`stack_position`; the chain-in-name is a hint. (`@`/`:`
  rejected for branches; `/` remote-only.)
- **Wave home is permanent.** Land never rotates; merged workers self-prune via the
  `delete`-branch webhook (shipped #818).

## Resolved forks (converging — confirm)

| Fork | Answer | Why |
|---|---|---|
| **Worker shape** | **b1** — thin async runner; each `WorkerMove` is a one-shot `lf` Exec in the Worker's worktree; Worker never touches `run_mind` | matches shipped `worker.rs` + its `questions.md`; **and dissolves the contradiction below** |
| **Process topology** | **2b** — Workers attach to the Wave's one listener; no per-Worker listener | if shape=b1, `server.rs`/`wire.rs`/`resident.rs` change *nothing* — the Worker is a client of existing doors |
| **Cascade trigger** | **poll + internal event, NOT a webhook** | no webhook infra exists (localhost daemon); the Worker loop already polls (`WaitForChecks → lf op pr`); the heartbeat already re-folds in-flight workers |
| **Dispatch target** | **collapse Fresh/Stack into fork-from-`parent()`** | `WaveId::parent()` already yields it; `ensure_wave_worktree` already pushes the wave branch |

**The contradiction b1 resolves:** DATAMODEL implies a Worker (reused resident)
*owns* chat/memory; `worker-lifecycle.md` says a Worker is **pen-transparent** —
no chat, no memory, children's speech/memory *bubble past it* to the owning wave.
A b1 Worker owns no listener and no `run_mind`, so it is pen-transparent **by
construction**. Choosing b1 makes the two docs agree.

## The one genuinely-open decision

**Merge authority.** Does a Worker land itself (`lf op land`, arms auto-merge) or
`lf op submit` and a human clicks? `worker.rs` defaults to `Submit`. This is the
trust call — the only fork the research says gates unattended operation.

## Hardening the design has NOT absorbed (prior-art gaps)

The design nails the happy-path cascade + 3 edge cases but is missing two mature
poles — **OTP's restart ladder** (bound retries per time window, then kill the
subtree and propagate up) and **Anthropic's effort-scaling** (bound fan-out by
complexity). Highest-leverage, in order:

1. **Per-subtree budget + max-restarts-in-window.** No stack-wide termination
   guarantee today; only per-Worker "bounded retries" (unnamed bound). Graphite hit
   a literal ~12h infinite CI loop on stacked merges. Kills gaps 1/2/9 at once.
   (`spend_cap` exists at Wave level, deferred to M4.)
2. **Recursion-depth cap.** LangGraph 25 / CrewAI 25 / OpenHands ~100 all cap
   precisely to prevent infinite spawn. **Footgun:** LangGraph's limit doesn't
   propagate to subgraphs — relevant to our open Q "does `workers:` bound the
   subtree or just direct children?" (must bound the subtree).
3. **Flaky-vs-real gate on Fix.** ~84% of pass→fail at Google is flakiness. Blindly
   "fixing" a red build burns billed CI and can mask real problems. Want
   reproduce-then-fail + quarantine, not blind retry.
4. **Non-convergence detection.** Fingerprint each iteration `(tool,args,result)`;
   exit on K identical / no-change-for-N *before* the hard cap. (Our own free-energy
   brief flagged this "progress setpoint" and deferred it to M4.)
5. **Fan-out / spawn-storm cap + cost accounting.** Multi-agent burns ~15× tokens;
   Anthropic's fix was orchestrator-prompt effort rules + a per-window spawn cap.
6. **Class-based escalation.** Escalate on *class* (arch decisions, high-risk files,
   conflicting reviewers) on sight, not only on retry-count exhaustion.
7. **Merge-queue cascade.** DATAMODEL disposes of it in one sentence. Real queues
   have speculative re-test cascades, rebase avalanches (N² + thundering herd →
   need batching + backoff-with-jitter), retry amplification (need a shared budget),
   and cross-sibling file-overlap conflicts (Aviator "affected targets"). Compare
   the bespoke "sealing" flag against Mergify's battle-tested "dependent waits for a
   later batch" before shipping it.
8. **Idempotency keys** on Work/Fix/Land (at-least-once retries can double-apply a
   commit/PR) and a **durable poison-PR bucket** (dead-letter, not `exit 1`) for a
   Worker that burns its budget — ceiling should be low (CI-fix is an expensive op).

## Already covered — don't re-solve

Lineage-in-metadata; parent-before-child ordering (cascade lands bottom-up by
construction); cycle prevention (stack is a DAG); deterministic `lf op rebase`
classifier; `lf chat --parent` as the escalation *channel* (but not a durable
poison bucket).

## First buildable slices (decision-free, from DATAMODEL staged plan)

1. **Dispatch target = `parent()`** — `create_run_for_placement`/`create_run_worktree`
   Fresh forks from + targets `<user>/<wave>`; merge Fresh/Stack; add fork tests
   (`fresh_forks_from_wave_branch_not_main`, `bare_wave_fresh_falls_back_to_main`).
2. **Exec = Run minus worktree** — Execs run in the Worker's worktree; `Run` loses
   its own tree.

Then the b1 runner around `worker.rs`, then the cascade (`Sealing` state + the
spawn-time guard, checked under store serialization — the one real race).
