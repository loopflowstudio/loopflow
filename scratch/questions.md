# W2-175 — open questions & blockers

## Environment blockers (reported, not fixing as prerequisites)

- **`lf task acknowledge W2-175` cannot run here.**
  - The installed `lf` on PATH is `0.11.1` and is *diverged* from the shared
    `~/.lf/loopflow.db`: the db carries migration `0.11.009_profiles` (from #936)
    which that binary does not know. This is the cross-branch migration-collision
    hazard already recorded in wave memory.
  - The locally built `target/debug/lf` (from this worktree, which is rooted after
    #936) reads the db past the migration gate, but reports
    `no Task Session exists for "W2-175"` — the Task Session
    `ts_44e584330d5c47a09c68bf12dad708ee` is not registered in this machine's store.
  - Per loopflow operating rules I report this once and continue inline. The
    directive acknowledgement summary is captured in this branch's design note
    instead; the design work does not depend on the store.

- **Host disk was at 100%** (1.5 GiB free of 926 GiB) at run start, which broke
  Bash output capture (`ENOSPC` writing tool output to `/tmp`). Freed transient
  `claude-501` session caches to ~9 GiB to unblock tooling. Underlying disk
  pressure is a machine-health issue, not caused by this task.

## PR state (pursue)

- `lf pr open` **rebased onto latest main and pushed the branch** (HEAD
  `409644d4b`), auto-resolving a conflict with #945's kickoff-iterate-gate
  phase-plan refactor; the rendezvous wiring composes cleanly with the phase model
  and all tests/fmt/clippy pass on the rebased tree.
- **PR creation itself did not complete**: `lf pr open`'s store step errored with
  `ambient Task Session ts_44e5843…  is not registered`, and GitHub's GraphQL quota
  was momentarily exhausted.
- **Root cause (mutate invocation, confirmed):** the session row *is* present in
  `~/.lf/loopflow.db` (`strings` match), and the orchestrator's own
  `/tmp/loopflow-compat-46c8134c-*` binary resolves `W2-175` → the session fine.
  But the two `lf` binaries available to me here do **not**: the installed
  `lf 0.11.1` fails the db's migration gate, and the worktree-built
  `target/debug/lf` (current rebased HEAD) reports `no Task Session exists for
  "W2-175"` / `ambient Task Session … is not registered`. This is a post-#936
  ("connect managed accounts through profiles") store-resolution divergence — the
  compat binary reads the store where the session is registered; the binaries I
  can run resolve a different (profile-scoped) path. Fixing that is store/auth
  resolution, which the operating rules direct me not to repair as a prerequisite.
- **Handoff:** the change is complete, reviewed, tested, and pushed (origin
  `409644d4b`, rebased onto main). The Task runner, driving the orchestrator's
  correctly-resolved binary, opens the PR via the same `lf pr open` that already
  rebased+pushed this branch. A raw `gh pr create` was deliberately **not** run —
  it would bypass loopflow's PR reconciliation and risk a duplicate. The pushed
  branch is the handoff point.

## Design decisions resolved inline (executive calls)

- **Scope of this PR = the runtime rendezvous only.** The shared store + CLI +
  DTO handoff contract already merged as PR 1 (#935); flow interaction policy
  (#941) and task resume position (#942) merged separately. This PR wires the
  existing handoff store into the Task body's block/resume — the piece that is
  genuinely missing (`WaitInteractive` is emitted by the engine but consumed
  nowhere).

- **Task parent first; Project/Wave wiring is a follow-up.** The rendezvous helper
  is written parent-agnostic (the handoff parent enum already spans wave/project/
  task), but only the Task runner is wired and tested in this PR. Mac Active
  Sessions and external presentation adapters are explicitly deferred (per the
  Task's delivery note).

- **Failed handoff routes the parent to `Blocked`, not a silent retry loop.**
  Completed / HandedBack advance the flow past the interactive step; Failed wakes
  the parent once and parks it `Blocked` (operator-resumable) with the failure
  reason, so an unmet interactive obligation never loops or is silently skipped.

## Known limitations of the shipped slice (follow-up)

- **Claim-then-crash-before-advance window.** The runner's birth reconcile claims
  the wake, then advances the flow cursor, then persists it. A crash in the gap
  between the claim and the cursor persist leaves a terminal-claimed handoff with
  the cursor still on the interactive step. `pending()` excludes claimed handoffs,
  so the next body resolves `None`, re-runs the interactive step, and the agent
  opens a *duplicate* handoff. Rare (a crash in a two-write window) and
  self-correcting (a human completes the duplicate). Closable with a step key on
  the handoff row, or by making claim+advance a single atomic store write. Not
  worth the schema/store surface in this first slice.

- **Project / Wave parents don't auto-wake on `lf handoff complete`.** The wake
  enqueue is Task-only for now; Project/Wave resume through their own supervision.
  The rendezvous *reader* is parent-agnostic, so wiring their runners is additive.

- **Flow-authored `WaitInteractive` does not auto-open a handoff.** This slice is
  agent-initiated: the agent opens the handoff with `lf handoff open`, and the
  runner reads it. Auto-opening at a `Require`-policy interactive flow step (so a
  flow author, not the agent, drives the rendezvous) needs the runner to build the
  attach descriptor itself and is deferred.
