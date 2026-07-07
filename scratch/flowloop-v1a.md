# flowloop v1a — research findings (R1, R2, R3) + proposal

> **SUPERSEDED by `scratch/flowloop-run2.md`.** Run 1 shipped this doc's v1a
> scope (`lf task` + `flowloop/task.rs`). Its two deferrals — the mind→flowloop
> renames and the "wave re-expression is v3" call — were overruled: run 2
> builds the tier-generic runtime, converts the wave, and lands the renames.
> The R1/R3 findings below (Linear oracle, budget backstop) remain valid.

Companion to `scratch/flowloop.md` (the charter). This is the design doc for
the first PR: research answers with file refs, then the v1a build.

## R2 — the flowloop runner vs `run_mind`: **neither replaces nor hosts**

The charter asked which is the smaller change. The code says both options are
wrong for the task tier:

- `run_mind` (`wave/mind.rs:502`) is a scheduler around a **persistent vendor
  thread**: biased select over inbox / harness events / heartbeat / cron /
  interrupt deadlines. Its whole machinery (EventAdapter → ResidentDelta wire,
  listener journal, SSE, steer-mid-turn) exists to keep one long conversation
  alive. `MindEnd` has exactly two arms — ListenerGone, Failed. There is no
  phase structure and no terminate arm to hook.
- The flow engine **already has a loop primitive**: `run_loop`
  (`engine/execution.rs:282`) runs `body → exit-router → path`, repeating until
  the `done` path. But its exit is **agentic** — `build_router_step`
  (`execution.rs:353`) synthesizes an LLM router that writes a verdict to
  `scratch/route-xor.md` — and it has **no iteration cap**. The deterministic
  oracle can't live there without teaching the engine op-routers + caps.

**Proposal: the task flowloop is a thin Rust runner outside both.** This is the
worker brief's b1 shape, with flow passes instead of `WorkerMove`s:

```rust
// new: rust/loopflow/src/flowloop/task.rs (~300 loc)
loop {
    // one pass: clarify → pursue_goal → mutate, as a plain 3-step
    // builtin flow, run headless in a child process
    run_pass(&worktree, "task", timeout)?;   // `lf task-pass -b`, kill on timeout
    if oracle_pr_merged(&worktree, pr)? { break }   // gh pr view --json state
    caps.check()?;                            // max passes, wall clock → escalate
}
```

What this buys, all from existing machinery:

- **Phases run bounded and headless** exactly like fork branches already do:
  `cmd.arg(step).arg("-b")` + child process (`lf/commands/flow.rs:634`). The
  hard backstop is trivial — `wait_timeout` + kill on the child.
- **Chat reaches the flowloop at phase boundaries for free.** Each pass is a
  fresh `lf` invocation with full context assembly, so the wave-chat snapshot
  (`<lf:wave-chat-recent>`, `engine/wave_context.rs`) re-folds every pass. No
  listener, no wire — pen-transparency by construction (decision #5).
- **Dispatch/worktree/tmux plumbing exists**: `create_run_for_placement`,
  worker naming `<repo>.<wave>.<short-run-id>`, exit files
  (`lfd/executor/helpers.rs`). Merged-branch self-prune shipped in #818.
- `run_mind` is untouched; the wave tier keeps its residency. Whether the wave
  is later re-expressed on this runner is a v3 question, not v1.

**Consequence for the §4.1 renames:** renaming `wave/mind.rs → wave/flowloop.rs`
now would be wrong — `mind.rs` is the *wave residency*, and the flowloop runner
is a new module. Proposal: land the new code as `flowloop/`, defer the
mind renames to the wave-tier unification. (Charter edit needed if agreed.)

## R1 — Linear as the KR oracle: **native, no fallback needed**

- Machine-readable "done" exists today: `PmItem.completed: bool`
  (`lfd/pm/mod.rs:61`), derived from Linear workflow-state
  `type == "completed"` (`lfd/pm/linear.rs:91`, `:474`). Linear's state *types*
  (backlog/unstarted/started/completed/canceled) are stable across custom
  workflows — the oracle reads the type, never the state name.
- The **project-level oracle already has an implementation**: `lf op pm status`
  computes `open/total` per wave project (`ops/pm.rs:73` `PmWaveStatus`).
  "All KRs done" ≡ `open == 0` over the KR items. For v2, KRs = issues in the
  project (optionally labeled `kr` to separate them from tasks); milestones
  exist in Linear but nothing in `pm.rs` touches them — not needed.
- **v1a task oracle: PR merged, alone.** The exact poll exists:
  `pr_state()` (`ops/next.rs:141`) — `gh pr view <n> --json state -q .state`,
  compare `MERGED`. Flipping the Linear item to done
  (`lf op pm update --id … --status done --pr <url>`) is a **mutate-arm
  action after the oracle fires**, not part of the oracle — GitHub is ground
  truth, Linear is bookkeeping.

## R3 (surfaced by the research) — the charter's budget assumption is wrong

`-b` is `--batch` (headless), **not budget**. No spend or wall-clock
enforcement exists anywhere on the run surface. `AgentConfig.max_turns`
plumbing reaches the harness (`engine/agent.rs:99,236`) but every call site
hardcodes `None` (`lf/commands/run.rs:135`, `wave/mind.rs:249`).

v1a backstop, two deterministic levels as the charter wants:
1. **Hard floor the agent can't override**: the runner's child-process
   wall-clock timeout + kill, and a max-passes cap.
2. **Soft**: wire `max_turns` through for phase runs (small change, real cap).

Dollar budgets are v1c; the charter's "§2 enforcement" wording should say
turns + wall-clock, not `-b`.

## v1a — what to build

**One command, happy path only:**

```
lf task <linear-item-id> [--wave <wave>]
```

1. Resolve the Linear item via the wave's PM project (`ops/pm.rs` context) —
   title + description become the task statement.
2. Create the ephemeral worktree/branch through existing dispatch placement.
3. Loop: one pass = builtin flow `task` = `task_clarify → task_pursue →
   task_mutate` (three builtin steps, the task row of the §3 skill matrix),
   headless, timeout-bounded, in the task's worktree.
4. After each pass, poll the oracle: no PR yet → continue; PR open → continue
   (waiting ≠ thrashing: if the tree is clean and CI is pending, sleep instead
   of re-passing); PR `MERGED` → close out.
5. Close-out: `lf op pm update --id <id> --status done --pr <url>`, exit 0.
   Worktree self-prunes when the branch is deleted (#818).
6. Caps (v1a minimal): `--max-passes` (default 8) + wall-clock (default 2h).
   On cap: `lf chat --parent` + exit nonzero. No fix-loop intelligence yet —
   that's v1b.

**Merge authority:** `Submit` (charter default). The loop drives the PR to
ready+submitted, then polls; the human's merge click is what flips the oracle.

**The three skills** live as builtins (with `engine/builtins.rs` siblings),
stating artifact, move menu, and oracle concretely per the charter's §3 matrix:
- `task_clarify` — read the Linear item + `scratch/<branch>.md`; make the
  design doc computable (write it if missing); noop when already clear.
- `task_pursue` — one pass of work toward the PR: code, tests, `lf op pr`.
- `task_mutate` — check state honestly: submit when done (`lf op submit`),
  note blockers to `scratch/questions.md`, never claim done (the oracle does).

**Demo:** file a real small task in Linear, run `lf task <id>`, walk away.
Come back to a submitted PR; click merge; the process notices, marks the
Linear item done with the PR link, exits 0, and the worktree is gone.

**Done when:** the demo above on a real task, plus
`cargo test flowloop` covering: oracle poll parses MERGED/OPEN/missing;
caps fire and escalate; pass runner kills on timeout.

## Open for Jack

1. Runner shape confirmed as b1-with-flow-passes (this doc), or push the loop
   into the flow engine (op-exit-routers + caps) instead?
2. Defer the `mind → flowloop` renames to wave-tier unification? (§4.1 says
   they ride v1.)
3. Command name: `lf task <id>` collides conceptually with `SessionUse::Worker`
   naming — fine to introduce `task` now and rename the session role in the
   same PR, or keep the role as-is until v1d?
