# Worktree redesign: recursive identity, stable wave homes, no land rotation

Fixes the fallout from #802 (plan-first placement). Agents ended up in paths
like `loopflow.jack-heart.bugs.20260705_1627.goals` with every wave-scoped `lf`
op blocked.

## What broke

1. **Runaway nesting.** `lf op wt create goals` with no flag builds
   `PlacementRequest::Default`. Off the default branch, `Default` treats the
   current branch as a *stack parent* and appends the segment
   (`worktrees.rs:644`). `stack_worktree_path` then sanitizes the whole dotted
   branch into the dir name. Each `wt create` on a non-main branch deepens it.

2. **Wave identity stops resolving.** `wave_name_from_worktree_and_main` strips
   the `loopflow.` prefix and returns the rest verbatim, so the "wave name"
   becomes `jack-heart.bugs.20260705_1627.goals` — matching no `wave/<name>/`
   dir. `lf chat`, `lf op pm`, rotation all misfire. `rotate_worktree` also
   hard-bails on any `.` in the wave name (`land.rs:481`). That is the "blocked."

3. **Land rotation moves the worktree out from under the agent.** `land` →
   `rotate_worktree` renames the live worktree to `<name>.<ts>` and spawns a
   fresh empty `<repo>.<wave>`. A CI-fix then means chasing the moved dir, and
   agents don't cope with their cwd being renamed mid-run.

4. **The dot is overloaded three ways** — `{user}.{name}.{timestamp}` branch
   schema, `seg.seg.seg` stack ancestry, `<repo>.<wave>.<run-id>` dirs — so
   nothing parses once nested.

## Vocabulary (settled 2026-07-06)

Three roles, two of them the same primitive:

| Role       | What it is                                                          | Owns                          | Branch example        |
|------------|---------------------------------------------------------------------|-------------------------------|-----------------------|
| **Wave**   | Persistent looping mind, arbitrary goal. Ships many PRs over time.   | a chain node (worktree+branch)| `jack/bugs`           |
| **Worker** | Ephemeral looping mind. Goal: land one PR to its parent, then dies.  | a chain node (worktree+branch)| `jack/bugs.fix@ts`    |
| **Exec**   | One arbitrary `lf` invocation (step/op/flow). No mind, no branch.    | nothing — runs in the mind's worktree | — (`lf debug`) |

Wave and Worker are the **same primitive** — a recursive, goal-directed looping
mind. The name marks the role: the root you point at a goal (Wave); every
descendant carries one PR up the stack (Worker). So:

- **A chain segment ⇔ a mind ⇔ one PR-to-parent.** Execs don't get chain
  segments; they run inside a mind's worktree and mutate its branch.
- **Dispatch is one rule: fork from, and target, `parent()`.** A Worker's PR
  targets its parent mind's branch; the root Wave's PR targets `main`. Infinite
  tiers fall out of the recursion — no per-level special-casing. `WaveId::parent()`
  already encodes it (`None` ⇒ target `main`).
- **The Worker-mind tier is the missing layer.** Today the Wave mind dispatches
  `lf` runs (Execs) directly; nothing owns the "get this one PR landed to my
  parent" loop (change → PR → CI fails → fix → retry — inherently a loop, so a
  mind). Building that runtime is stage 3b.

## Identity model — SHIPPED (`WaveId` in `engine/identity.rs`)

One thing, **two decoupled projections** that are not string-derivable from each
other. The research killed `@` (footgun, outside GitHub's safe set) and pointed
at `/` for the remote — but `/` can't go in a worktree dir (it'd nest). So the
dir stays a flat `.`-chain and the remote branch carries `/` + author. The
dir↔branch link lives on the `Run` record, not string surgery.

| Projection    | Wave              | Worker                              |
|---------------|-------------------|-------------------------------------|
| Wave name     | `bugs`            | `bugs` (chain segment 0)            |
| Chain         | `bugs`            | `bugs.fix-auth.retry`               |
| Worktree dir  | `loopflow.bugs`   | `loopflow.bugs.fix-auth.retry.20260706_0801` |
| Remote branch | `jack/bugs`       | `jack/bugs.fix-auth.retry.20260706_0801` |

Rules:

- **`dir_component()` = `{chain}[.{ts}]`** — flat, author-free, local. Dir is
  `{repo}.{dir_component}`.
- **`branch()` = `{user}/{chain}[.{ts}]`** — author-scoped (`/`), glob-able
  (`jack/**`), never on disk.
- **`parse(raw, fallback_user)`** is the single input funnel: accepts either
  form (with/without `user/`, with/without trailing stamp), fills user from the
  fallback when absent. Postel: liberal in, strict out.
- **Wave name = chain segment 0.** Keys `wave/<name>/`, chat, pm.
- **Waves/subwaves are stamp-free; workers carry one trailing `.ts`**, minted at
  dispatch, always last. `a.b` worker `→ a.b.c.<new ts>` (re-stamp at the tail).
  The stamp's presence *is* the worker/subwave marker.
- **Stamps come from dispatch, not `wt create`.** Human `wt create` makes
  persistent (unstamped) nodes; the executor makes stamped workers.

Parsing (deterministic, no regex shape-guessing):

```
raw.split_once('/')  -> [user?, rest]     user before '/', else fallback
rest.split('.')      -> segments; trailing YYYYMMDD_HHMM popped as the stamp
segments[0]          -> wave name
```

Rejected delimiters: `@` (legal in refs but outside GitHub's safe set, shadows
`@{upstream}`, breaks pip/CI URL parsing); `:` (illegal in refs, macOS Finder
renders it `/`).

The decoupling (dir = flat `.`-chain, author-free; branch = `/`+author,
remote-only) is settled and shipped. The supporting branch-format research
(Graphite/Sapling/ghstack/gh keep lineage in metadata not names; `@` is a legal
footgun; `/` breaks the flat sibling-dir invariant) is folded into
`wave/systems/MEMORY.md`.

## Land redesign (kills rotation)

- The wave lives permanently in `<repo>.<user>.<wave>` on branch
  `{user}.{wave}` (stamp-free integration branch). **Never rotated.**
- PRs are owned by dispatched **subworkers**, one per PR, each in its own
  `@ts` worktree. A subworker owns the whole arc: commit → PR → land →
  `lf-ci-fix`. When it lands, only its own worktree is preserved/pruned — the
  wave home stays put.
- Direct commits from the wave home stay *possible* (active firefighting) but
  are discouraged as SOP. Soft guidance in LOOPFLOW.md, not a hard block.
- Remove `rotate_worktree` (land.rs), `advance_branch` / `next_branch`
  rotation (next.rs). Land from a worker worktree just lands and prunes itself.

## Placement (point 1) — DONE

Two relative-to-here verbs replace the old four flags:

- **`--sibling`** (the default, no flag needed): root an independent branch off
  main. Was `--main`/`--fork`.
- **`--child [PARENT]`**: stack under `PARENT`, or under the current branch when
  omitted. Was `--stack`/`--base`.

Shipped: default no-flag `wt create` roots from main (kills the runaway nesting);
`PlacementRequest::Default`/`Fork` deleted; docs + builtin LOOPFLOW.md + goldens
regenerated. `PlacementRequest` internal variants stay `Main`/`Stack` for now;
stage-2 aligns the vocabulary.

## Dispatch kind: worker vs subwave

Dispatch has a *kind*, orthogonal to placement:

- **Worker** (all we launch today): ephemeral task executor. Gets the trailing
  timestamp — `bugs.fix-auth@…20260706_0801`. Stamp = "this leaf is ephemeral."
- **Subwave** (future): a persistent sub-*mind*, not a task. Extends the chain
  **without** a timestamp — `bugs.triage` — because waves aren't ephemeral.

So the timestamp's presence *is* the worker/subwave marker, and it falls out of
the existing rule (waves stamp-free, workers stamped) for free. For now every
dispatch is a worker; the CLI just needs a spot to say `subwave` later without
reshaping identity.

## Staged plan

1. **Placement fix — DONE** (`d722deb8`). `--child`/`--sibling`, root-from-main
   default, `Default`/`Fork` removed.
2. **Chain-aware naming — DONE** (`f775aef0`). `WaveId` type; decoupled
   `dir_component()`/`branch()`; `BranchNameConfig`/`StackBranch` retired; all
   callers ported.
3. **Land redesign — IN PROGRESS.**
   - 3a. **Kill land rotation — DONE.** `rotate_worktree`/`RotationResult`/the
     land-cd removed. A land never renames the live worktree; the wave home is
     permanent. Workers self-prune once merged.
   - 3b. **The Worker-mind runtime — TODO (new subsystem, not a mechanical edit).**
     Insert the Worker tier: a looping mind per PR whose goal is "land to
     `parent()`", that dispatches Execs (`lf` runs) in its own worktree and
     retries through CI. Dispatch collapses to fork-and-target `parent()`
     (decided: worker PRs target the parent branch, `jack/bugs`, recursively to
     `main`). Retire the old ephemeral rotation (`lf op next`/`advance`,
     `next_wave_handler`) — a Rust + wire + Concerto change.
4. **Docs + goldens + tests.** Per slice.

## Stage 3b open questions (before building the Worker runtime)

- **Integration trigger (per tier).** Once a Worker's PR merges into its parent,
  what lands the parent up to *its* parent — cascading to `main`? Lean: a mind
  lands into its parent automatically once its own PR merges *and* it has no
  unlanded children (self-draining bottom-up). Confirm vs. manual/threshold.
- **Worker vs Wave in the runtime.** Same primitive — how much of the Wave mind
  loop (`wave/resident.rs`, `mind.rs`) is reused for a Worker with a fixed
  land-to-parent goal vs. an arbitrary one?
- **Exec = Run minus worktree.** Today a `Run` gets its own worktree; an Exec
  should run in its Worker's worktree with no new branch. Collapses per-Run
  worktrees into per-Worker.
- **Retiring `next`/`advance`.** `next_wave_handler` is a wire endpoint Concerto
  calls. Removing it is a Rust + wire + Swift change. Confirm before cutting.
- **Subwave dispatch.** A CLI/dispatch flag to spawn a subwave-mind (unstamped
  descent) vs. a worker (stamped). Identity already supports both.
