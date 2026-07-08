# Wave / Worker / Exec — the wave data model

The entities the wave runtime is built on, and the Worker tier we're adding
between the Wave mind and raw `lf` runs. Identity types live in
`crate::engine::identity` (`WaveId`); the mind runtime is `resident.rs` +
`mind.rs`; dispatch is `lfd/executor`.

## The three roles

- **Wave** — persistent mind, arbitrary goal, lives in `<repo>.<wave>` on branch
  `<user>/<wave>`.
- **Worker** — ephemeral mind, goal *"land my PR to my parent"*, lives in a
  stacked worktree on branch `<user>/<chain>.<ts>`.
- **Exec** — one `lf` invocation (step/op/flow) a mind runs in its own worktree.
  No mind, no branch.

## The gap today

The Wave mind (`wave/resident.rs` → `wave/mind.rs::run_mind`) already exists: a
harness conversation attached to a listener, running in the wave worktree. It
dispatches work by shelling `lf <flow> --dispatch`, which creates a `Run` —
a one-shot agent flow in its own worktree (`lfd/executor/helpers.rs::create_run_for_placement`).

What's missing: **nothing owns the loop that gets one PR landed to its parent.**
A `Run` does the work once; it doesn't watch CI, fix a red build, resolve a
rebase conflict, and retry until the PR is merged. That shepherding is a loop,
so it wants a mind — the Worker.

## The Worker as a resident, reused

A Worker is the **same primitive as a Wave** — so it's the same runtime:
`resident.rs`'s attach → `run_mind` loop, spawned in a stacked worktree, with

- a **fixed, generated goal** instead of a human `GOAL.md`: *"Land the PR for
  `<chain>` into `<parent-branch>`. Use Execs to do the work; open the PR, drive
  it green, land it, then stop."*
- an **ephemeral lifecycle**: it exits when its PR is merged (success) or when it
  escalates to its parent and is told to stop (giving up).

Reusing the resident means Workers inherit steering, interrupt, chat-to-parent,
memory, and the wire — for free. The parent mind is "the human" for its Workers:
`lf chat --parent` is already the escalation channel.

## The Worker loop (state machine)

The loop is mostly deterministic; the mind is invoked only where judgment is
needed. States:

1. **Work** — dispatch Execs that implement the change (`lf implement …`,
   `lf debug …`). This is where the LLM writes code. Commit as it goes.
2. **PR** — `lf op pr` targeting `parent()` (the parent mind's branch; `main`
   for the root Wave). `WaveId::parent()` already yields this.
3. **Watch** — poll CI (`lf op wt ci`). Green → Land. Red → Fix.
4. **Fix** — dispatch a fix Exec (`lf op` rebase agent for conflicts, an
   `lf debug` for a red build), then back to Watch. Bounded retries.
5. **Land** — merge the PR into the parent branch, prune this worktree, exit.
6. **Escalate** — retries exhausted or blocked: `lf chat --parent`, mark the
   Worker blocked, exit nonzero. The parent decides (retry, reassign, drop).

**Key decision — how much mind?** Two shapes:

- **(a) Full mind in the loop.** The Worker is a `run_mind` conversation the
  whole way; the states above are its instructions. Most flexible, most tokens.
- **(b) Structured loop, mind on demand** *(recommended)*. The state machine is
  plain Rust; it invokes a mind (an Exec that launches an agent) only for Work
  and Fix — the judgment steps. Watch/PR/Land/Escalate are mechanical. Matches
  "just do explicit steps/ops/flows" and the existing pattern where `lf op land`
  launches a rebase agent only on conflict (`ops/land.rs`). Cheaper, more
  legible, easier to instrument.

Recommendation: **(b)**. The Worker is a supervisor loop that spends tokens only
on Work and Fix. It can still *escalate to a full mind* if a Fix loops without
converging.

## Execs = Run minus worktree

Today each dispatched `Run` gets its own worktree. Under the new model, an Exec
runs **inside its Worker's worktree** and mutates the Worker's branch — no new
branch, no new chain segment. So:

- The per-`Run` worktree collapses into the **per-Worker** worktree.
- `Run` becomes the record of one Exec (an `lf` invocation): its command, cwd
  (the Worker's worktree), status, output pointer. It keeps `parent_run_id` for
  the Exec sequence but no longer owns a `worktree`/`branch` of its own — those
  belong to the Worker.
- Only **Workers** (chain nodes / minds) create worktrees + branches, via
  `worker_id()` + `worktree_dir()`.

## Dispatch = fork-and-target `parent()`

One rule, all tiers:

- A Worker forks its branch from, and targets its PR at, its parent's branch.
- `WaveId::parent()` returns that branch; `None` (a bare wave) ⇒ target `main`.
- `Placement::Stack` already forks from the parent run's branch. `Placement::Fresh`
  becomes "child of the wave" — fork from and target `<user>/<wave>`. The two
  placements collapse into "fork from `parent()`".

The parent branch must exist on origin before a child PR can target it —
`ensure_wave_worktree` already creates and pushes `<user>/<wave>`; a Worker
ensures its own branch is pushed before spawning children (it already is, via
`schedule_upstream_sync`).

## The self-draining cascade (integration trigger)

The rule that makes the infinite stack reach `main`, applied at every tier:

> A mind lands its own PR into its parent **as soon as its PR is approved/green
> AND it has no unlanded children.** Landing a child removes it from the parent's
> unlanded set; when the last child lands, the parent becomes eligible and lands
> into *its* parent. The stack drains bottom-up with no manual step.

Worked example — `bugs` with children `fix-auth` and `retry` (a child of
`fix-auth`):

```
retry green, no children      → retry lands into fix-auth
fix-auth green, retry landed  → fix-auth lands into bugs
bugs green, fix-auth landed   → bugs lands into main
```

Edge cases to handle:

- **Sibling order.** Two green children of the same parent land serially (the
  merge queue / rebase-on-land already serializes); the second rebases onto the
  first.
- **Parent moves under a child.** When a sibling lands into the parent, open
  children rebase onto the new parent tip (the Worker's Watch state re-runs
  `lf op rebase`, which we already have).
- **A child arrives after the parent went green.** The parent is only eligible
  while it has *no unlanded children*; a newly-spawned child makes it ineligible
  again until that child drains. So the wave mind must not spawn into a parent
  that is mid-land. Cheap guard: a parent marks itself "sealing" before it lands;
  spawns into a sealing parent are refused (spawn a sibling of the parent
  instead, or wait).

Alternative triggers considered: manual (`lf` command per level — too much
toil at depth) and threshold (land after N children — arbitrary, leaves work
stranded). Self-draining is the only one that scales with the recursion.

## What changes in code

- **New:** the Worker supervisor loop (shape (b)), reusing `resident.rs` attach +
  `run_mind` for the Work/Fix minds. Likely `wave/worker.rs`.
- **Spawn path:** the Wave (and every Worker) spawns children as Workers, not
  one-shot Runs. `lf <flow> --dispatch` becomes "spawn a Worker for this chain".
- **`Run` → Exec:** drop per-Run worktree/branch; Execs run in the Worker tree.
- **Dispatch target:** `parent()` for fork + PR base (replaces Fresh→main).
- **Retire rotation:** `lf op next`/`advance`, `next_wave_handler`,
  `combine_wave_handler` — the old ephemeral-rotation model. Touches the wire and
  Loopflow; stage on its own.

## Open decisions (need Jack)

1. **Worker shape:** (b) structured loop + mind-on-demand — confirm vs (a) full
   mind throughout.
2. **Process topology:** is every mind a listener+resident pair (uniform, heavy —
   a nested wave server per Worker), or do Workers attach to their parent's
   listener (one listener per Wave, lighter)? Leaning: Workers attach to the
   Wave's listener; the chat/steer tree is logical, not per-process.
3. **Self-draining cascade:** confirm as the integration trigger (recommended).
4. **Concurrency:** how many Workers run in parallel under one parent — the
   `workers:` GOAL.md frontmatter already exists; does it bound the whole subtree
   or just direct children?

## Staged build

1. **Exec/worktree collapse** — Execs run in the Worker worktree; `Run` loses its
   own tree. Contained to `lfd/executor` + types.
2. **Dispatch target = `parent()`** — Fresh forks from/targets the wave branch.
   Update `create_run_for_placement` + the fork tests.
3. **Worker supervisor loop** — the state machine (shape b), reusing the mind for
   Work/Fix. The heart.
4. **Self-draining cascade** — land-into-parent on green + no-unlanded-children,
   with the sealing guard.
5. **Retire rotation** — remove `next`/`advance`/rotation endpoints; wire + Swift.
