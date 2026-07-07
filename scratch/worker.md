# Worker: integrated implementation plan

Integrates four sources into one plan: `wave/DATAMODEL.md` (the spec in #818,
recovered — it was wiped from the tree on land), the original wave's
`worker-build-brief.md`, our design discussion, and the task-as-handoff idea.
Where they conflict, **§0 states the resolution; the rest of the doc is the
chosen plan, not a menu.**

---

## 0. The one architectural choice (decided)

**A Worker is a MIND, not a Rust state machine.** It reuses the resident +
`run_mind` runtime, seeded with a convergent goal and a deterministic termination
clause. The mind chooses its moves.

This **supersedes** the shape both prior sources recommended:
- `DATAMODEL.md` shape **(b)** — "structured Rust loop, mind-on-demand."
- the build-brief's **b1** — "thin deterministic runner, never touches
  `run_mind`" — which *is* the `worker.rs` we scrapped.

Why we diverge: we want the shepherding *judgment* (what to do about a red build,
a review, a conflict) to be the mind's, kept flexible and legible as a
conversation — not frozen into a `choose_move` match. **Cost, the reason those
sources went deterministic, is bounded three other ways instead:**
1. the mind dispatches **cheap Execs/ops** for the mechanical steps (open PR,
   poll CI, rebase, land) and spends tokens only on **Work** and **Fix**;
2. between steps it **idles on the existing heartbeat/select loop**, it does not
   busy-spin an LLM on "is CI green yet";
3. a **deterministic terminator** + **`-b` budget/timeout** cap the whole thing.

Everything else in DATAMODEL and the brief is **absorbed** (below). Only the
"state machine vs mind" fork is overridden.

**Scrap:** `rust/loopflow/src/worker.rs` (the b1 policy core) + its `lib.rs` line.
That deletion is already in the base tree; this branch starts from the cleaned
substrate and lands the S1 dispatch-targeting slice.

---

## 1. The model — Wave / Worker / Exec (from DATAMODEL, kept)

- **Wave** — persistent mind, arbitrary goal; lives in `<repo>.<wave>` on branch
  `<user>/<wave>`. Never terminates.
- **Worker** — **ephemeral mind**; goal *"drive one task to a terminal state,
  then die."* Lives in a stacked worktree on `<user>/<chain>.<ts>`.
- **Exec** — one `lf` invocation (step/op/flow) a mind runs **inside its own
  worktree**. No mind, no branch. (This is `Run` **minus** its worktree.)

Invariant: **a chain segment ⇔ a mind ⇔ one PR-to-parent.** Execs get no segment.

---

## 2. The handoff — a task in, a terminal state out (the terminator)

The unit of handoff is a **PM task** (a Linear issue — the roadmap is Linear now;
Asana was removed). A Worker is spawned *from* a task, by the wave-mind or by
`lf worker <task-id>`.

**Convergent goal:** drive the task to a **terminal state — complete or delete.**
- **Complete** usually means: land the PR to `parent()` **and verify it in
  production.**
- **Delete/cancel** means: the Worker judged the task invalid/obsolete and closed
  it. A legitimate terminal outcome — not every task ends in a merge.

**Termination is deterministic and non-agent — the heart of the design.** The
mind chooses moves; a **non-agent oracle** decides the halt: `if (task terminal)
stop`, read from **task status + `gh` PR state + a prod-verification check**. The
agent cannot fake completion or dark-room its way out — ground truth is
Linear/GitHub/prod, not the model's self-report.

**The terminator is a composable predicate**, so the same machinery generalizes:
swap the oracle for `spend ≥ $N` and the Worker becomes a budget-bounded explorer;
`AND` them for "land+verify, but stop at $N either way." The Worker is one
instantiation — **a mind + a deterministic terminator.**

**Enforcement (decided, revisitable):** put the termination clause **in the agent**
("poll the task / `gh`; stop only when terminal") and run with **`-b`**, so budget
+ wall-clock timeout are the **harness-enforced backstop the agent can't
override.** Purist alternative — an external terminator loop — deferred unless
self-halt proves unreliable (§9).

---

## 3. Decisions locked (the integrated ledger)

Our decisions + DATAMODEL's settled substrate + the brief's settled forks.

1. **Worker = a mind** (reuse `run_mind`/resident); move-selection is the mind's.
   *(§0 — supersedes DATAMODEL-(b) / brief-b1.)*
2. **Deterministic terminator; unit = a task.** Done ≡ task terminal (land+verify,
   or delete). In-agent clause + `-b` backstop. *(§2)*
3. **Exec = Run minus worktree.** Execs run in the Worker's worktree and mutate
   its branch; only Workers (minds) own worktrees/branches. `Run` keeps
   `parent_run_id` for the Exec sequence, drops its own tree.
4. **Dispatch = fork-and-target `parent()`.** One rule, all tiers; `parent()==None
   ⇒ main`. Collapses `Placement::Fresh`(→main) and `Stack`(→parent).
5. **Lineage lives in metadata**, not the name — `Run.parent_run_id` /
   `stack_group_id` / `stack_position`; the chain-in-name is a hint. Stamps come
   from **dispatch**, not `wt create`. Waves stamp-free; workers carry `.ts`.
6. **Wave home is permanent.** Land never rotates; merged workers **self-prune**
   via the delete-branch webhook (shipped #818). Retire the old rotation model
   (`lf op next`/`advance`, `next_wave_handler`, `combine_wave_handler`).
7. **Topology 2b — one listener per Wave.** Workers **attach to the Wave's
   listener** (chat/steer tree is logical, not a nested server per Worker). No
   durable per-Worker pen: it is **pen-transparent** — children's chat/memory
   bubble up to the owning wave; the worker curates nothing that outlives it.
8. **Reporting.** Success = the **merged PR** (a durable fact the wave folds via
   `fold_workers`). Block = **`lf chat --parent`** escalation + nonzero exit; the
   parent mind decides (retry / reassign / drop). The wave sees **outcomes, not
   turn-by-turn activity** (that stays in the raw session).
9. **HITL = raw-session attach.** Humans watch/steer by attaching to the vendor
   terminal; no human chat surface *to* a Worker.
10. **Merge authority defaults to `Submit`** (human clicks merge); **`Land`
    (auto-merge) is opt-in per dispatch.** The one genuine trust fork (§9).
11. **Self-draining cascade is the integration trigger** (§5). Poll + internal
    event, **not a webhook** (no webhook infra; the loop already polls).

---

## 4. The Worker's move menu (the mind's playbook)

DATAMODEL's states become the mind's **move menu**, not a hardcoded machine. The
mind picks the move; mechanical moves are cheap Execs/ops, judgment moves spend
tokens.

| Move | How | Token cost |
|---|---|---|
| **Work** | dispatch an Exec that writes code (`lf implement` / `lf debug`), commit as it goes | mind (judgment) |
| **PR** | `lf op pr` targeting `parent()` (`main` for a root wave) | cheap op |
| **Watch** | poll CI (`lf op wt ci`); idle on heartbeat between polls | ~free |
| **Fix** | dispatch a fix Exec (rebase agent for a conflict, `lf debug` for a red build) | mind (judgment) |
| **Land** | merge into the parent branch, prune worktree, exit (see cascade §5) | cheap op |
| **Escalate** | `lf chat --parent`, mark blocked, exit nonzero | cheap |

The mind loops Work→PR→Watch→(Fix→Watch)*→Land, **but the loop is its judgment**,
gated by the terminator. It does not busy-spin: after dispatching, it idles until
an Exec finishes or CI resolves.

---

## 5. The self-draining cascade (the integration model — from DATAMODEL, kept)

The rule that makes an arbitrarily deep stack reach `main` with no manual step:

> A mind lands its PR into its parent **as soon as its PR is approved/green AND it
> has no unlanded children.** Landing a child removes it from the parent's
> unlanded set; when the last child lands, the parent becomes eligible and lands
> into *its* parent. The stack drains bottom-up.

Worked: `retry` (green, no children) → lands into `fix-auth`; `fix-auth` (green,
retry landed) → lands into `bugs`; `bugs` (green, fix-auth landed) → lands into
`main`.

Edge cases to handle:
- **Sibling order** — two green children of one parent land serially; the second
  rebases onto the first (merge-queue / rebase-on-land already serializes).
- **Parent moves under a child** — when a sibling lands, open children rebase onto
  the new parent tip (the Watch move re-runs `lf op rebase`, which #836 makes
  re-parent correctly).
- **Child arrives after parent went green (the one real race)** — a parent is
  eligible only while it has *no unlanded children*. A parent marks itself
  **`Sealing`** before it lands; **spawns into a sealing parent are refused**
  (spawn a sibling of the parent, or wait). The guard must be checked **under
  store serialization** — this is the single concurrency hazard to get right.

---

## 6. Implementation slices (buildable order)

Merges the brief's decision-free foundation with our minded-Worker build. Each
slice is independently landable and names its demo.

**S0 — Scrap `worker.rs`.** Remove the b1 policy module + `lib.rs` line.
*Done:* crate builds without it; branch reflects "design, no premature code."

**S1 — Dispatch target = `parent()`.** Collapse `Fresh`/`Stack` into
"fork-from-and-target `parent()`" in `create_run_for_placement` /
`create_run_worktree`; `None ⇒ main`. *Markers:* fork tests
(`fresh_forks_from_wave_branch_not_main`, `bare_wave_fresh_falls_back_to_main`).
*Demo:* a `Fresh` dispatch forks from `<user>/<wave>`, not main. **(Decision-free
foundation.)**

**S2 — Exec = Run minus worktree.** Execs run in the Worker's worktree; `Run`
drops its own tree/branch, keeps `parent_run_id`. *Demo:* two Execs mutate the
same Worker branch; no stray worktrees created.

**S3 — The minded terminating Worker.** `lf worker <task-id> --wave <name>` spawns
a resident/`run_mind` in a stacked worktree, seeded with the task goal + the
gh/task termination clause, run with `-b`. Reuses the wave listener (topology 2b).
*Markers:* boots a mind; opens a PR to `parent()`; **terminates by itself** when
the task is terminal (gh-gated, not model say-so); `-b` force-ends a runaway.
*Demo:* `lf worker <trivial-task>` → PR opened → task marked done on merge →
**process exits, worktree self-prunes**, unattended.

**S4 — Observation into the mind.** Each loop injects live task/PR/CI/review state
(via the exec door / `gh` / `lf op pm`) into context; mechanical moves are cheap
Execs. *Demo:* a red CI visibly redirects the next move within one loop.

**S5 — Bounded loop + hardening (the brief's prior-art gaps).** Per-subtree budget
+ **max-restarts-in-window** (OTP ladder); **recursion-depth cap that bounds the
SUBTREE** not just direct children; **non-convergence fingerprint**
(`(tool,args,result)` — exit on K identical *before* the hard cap); **flaky-vs-real
gate on Fix** (reproduce-then-fail + quarantine, not blind retry — ~84% of
pass→fail is flakiness); **idempotency keys** on Work/Fix/Land; a **durable
poison-PR dead-letter** bucket (not `exit 1`). *Demo:* an unfixable task exits
Blocked (escalated) after the cap, PR left open with a reason; a **slow-but-passing
CI still lands** (waiting ≠ thrash).

**S6 — The cascade.** `Sealing` state + the spawn-time guard under store
serialization; bottom-up drain; sibling rebase-on-land. *Demo:* a 3-deep stack
(`bugs`→`fix-auth`→`retry`) drains to `main` with no manual step.

**S7 — Prod verification in the terminator.** "Complete" = merged **+** a
deterministic prod check passes (deploy smoke / health / `verify` against prod).
*Demo:* a task is only marked Done after the merged change verifies in prod;
verification failure re-opens work, doesn't falsely complete.

**S8 — HITL raw-session attach.** Discoverable session name; `tmux attach` /
Concerto link into the live worker; no chat channel. *Demo:* attach mid-loop,
steer via the raw session, detach, it continues.

---

## 7. Definition of Done + acceptance checklist

**DoD — the feature is complete when all hold:**
1. `lf worker <task>` drives a task to a terminal state **fully unattended** —
   land+verify, or delete — and **terminates on its own** (gh/task-gated). No hung
   workers.
2. Deep stacks **self-drain** to main; the `Sealing` race is handled under store
   serialization.
3. A Worker owns only its worktree — **no durable channel/MEMORY**; children's
   chat/memory land in the **owning wave**; block escalates via `lf chat --parent`.
4. A Worker is a **raw session a human can attach to**; no chat surface.
5. `Submit` default; `Land` only on request.
6. The bounded loop **distinguishes waiting from thrashing**; unfixable → Blocked
   with a reason; slow CI → lands.
7. `cargo test` + `cargo clippy -- -D warnings` green; a smoke test covers
   open-PR-through-terminate.

**Acceptance (observable signals):**
- [ ] `lf worker <trivial>` → PR merged → task Done → **process gone**, worktree pruned.
- [ ] Kill switch never needed in a normal run.
- [ ] Unfixable task → Blocked, escalated, PR left open with a readable reason.
- [ ] Slow-CI task → **waits then lands**, never false-trips thrash.
- [ ] 3-deep stack → drains to main unattended.
- [ ] A memory fact from inside a worker → in the **wave's** MEMORY.
- [ ] `tmux attach` into a live worker works; no worker chat channel exists.
- [ ] Task marked Done only after **prod verification** passes.

---

## 8. Hardening backlog (prior-art poles the design must absorb, from the brief)

Ordered by leverage. These land in **S5/S6**, not later:
1. **Per-subtree budget + max-restarts-in-window** — no stack-wide termination
   guarantee today (Graphite hit a literal ~12h infinite CI loop on stacked
   merges). Kills the runaway class at once.
2. **Recursion-depth cap** — LangGraph/CrewAI cap at 25; **must bound the subtree,
   not just direct children** (LangGraph's footgun: limit doesn't propagate to
   subgraphs). Answers the open "does `workers:` bound the subtree?" (§9).
3. **Flaky-vs-real gate on Fix** — reproduce-then-fail + quarantine; blind "fix red
   build" burns billed CI and masks real failures.
4. **Non-convergence detection** — fingerprint each iteration; exit on K identical
   / no-change-for-N before the hard cap.
5. **Spawn-storm cap + cost accounting** — per-window spawn cap; multi-agent burns
   ~15× tokens.
6. **Class-based escalation** — escalate on *class* (arch decisions, high-risk
   files, conflicting reviewers) on sight, not only on retry exhaustion.
7. **Merge-queue cascade** — real queues need batching + backoff-with-jitter
   (rebase avalanche is N²), a shared retry budget, cross-sibling file-overlap
   detection. Compare the bespoke `Sealing` flag against Mergify's "dependent waits
   for a later batch" before shipping.
8. **Idempotency keys** on Work/Fix/Land (at-least-once retries can double-apply)
   + a **durable poison-PR bucket** (dead-letter, low ceiling — CI-fix is expensive).

Already covered — don't re-solve: lineage-in-metadata; parent-before-child
ordering (cascade lands bottom-up by construction); cycle prevention (stack is a
DAG); deterministic `lf op rebase` reparent classifier (#836); `lf chat --parent`
as the escalation *channel*.

---

## 9. Open questions (what still needs Jack)

1. **Merge authority — `Submit` vs `Land`.** The one trust call: does a Worker arm
   auto-merge itself, or stop at ready-for-human? Default `Submit`; flip per
   dispatch. Gates unattended operation.
2. **"Verify in production" as a deterministic oracle.** What concrete check marks
   a task truly Done — a deploy smoke test, a health endpoint, a `verify`-against-
   prod, a monitoring query? This is the hardest part of the terminator to make
   non-agent; until it's defined, "complete" leans on PR-merged alone.
3. **Subtree vs direct-children bound.** `workers:` / depth caps **must bound the
   whole subtree** (the LangGraph footgun). Confirm the cap propagates.
4. **Termination enforcement level.** In-agent clause + `-b` (lean) vs an external
   terminator loop (stronger, more machinery). Decide when S3 is built.

---

## 10. Source map

- **Spec:** `wave/DATAMODEL.md` — not in the tree; recovered from #818
  `098bbcaf:rust/loopflow/src/wave/DATAMODEL.md`. Its shape-(b)/topology decisions
  are resolved here (mind, 2b); merge-authority remains open.
- **Mind runtime to reuse:** `wave/resident.rs` (attach) + `wave/mind.rs`
  (`run_mind`: `while end.is_none()`, `MindEnd`, biased select, heartbeat,
  interrupt) + `wave/supervisor.rs`. Termination = a new `MindEnd::Converged/Blocked`.
- **Identity substrate (#818):** `engine/identity.rs` — `WaveId` with `parent()`,
  `is_worker()`, `depth()`, `chain_str()`, `dir_component()`/`branch()` projections.
- **Dispatch:** `lfd/executor/helpers.rs` (`create_run_for_placement`,
  `ensure_wave_worktree`, `schedule_upstream_sync`).
- **Session role exists:** `SessionUse::Worker` (`lfd/types/session.rs`).
- **Reparent-on-land (#836):** `ops/rebase.rs` (`merged_parent_fork_point`,
  `plan_rebase`) — daemonless, git-inferred.
- **Privileged verbs:** exec door `POST /v0/exec` (#825); scope to the Worker's
  verb set (door-review rec 2).
- **Roadmap item:** goals wave, "Worker: a terminating PR-landing mind"
  (`18603b7f-642e-4588-9622-23f8a15fd4f4`).
- **Superseded scratch (recover if needed):** `git show 7c67113a:scratch/worker-lifecycle.md`,
  the build-brief on the research worktree.
