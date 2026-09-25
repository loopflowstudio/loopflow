# Follow-up: software UX prototyping and Flow alternatives

2026-09-25. Task-ready research handoff, separate from LOO-295 implementation.
Filed as [LOO-297](https://linear.app/loopflow/issue/LOO-297). Current human
scope is `design -> review-design` versus `prototype -> review-prototype`, plus
a later `demo` or `concept-review` choice. Existing choose-one XOR integration
belongs to LOO-295. Parallel execution and variant comparison below are design
options, not additional accepted requirements. See fork-request.md.

## Problem and desired experience

Help a software creator discover how an interaction should work by trying it.
Create a specialized **software UX prototyping** skill that builds runnable
web/JavaScript demos with hand-built fake data. General design can shape a
problem or write a proposal; this skill's deliverable is an executable experience
that exposes navigation, state changes, feedback, and recovery behavior.

For a concrete interaction, build meaningfully different alternatives, try the
same scenarios in each, and carry the chosen behavior and remaining questions
into implementation. Include realistic populated, empty, loading, error, and
recovery states where relevant, with deterministic reset or scenario controls.
Fake data and simulated effects should be explicit; production services are not
prerequisites. A static screenshot, prose comparison, or cosmetic reskin alone
does not establish the interaction.

Re-explore forks only insofar as they improve this experience within today's
loopflows. Alternatives might be artifacts produced by one skill, independent
agent contributions, or parallel Flow branches. Do not revive the old executor
as a prerequisite to prototyping. Naming, syntax, storage, and execution placement
remain design questions.

## Historical findings

Evidence below is local `git show`/`git log`: merged commit bodies containing PR
descriptions, selected source, and historical test definitions. PR links identify
the changes; their live discussion was not fetched. Source links pin the inspected
revision. No archived JSON was opened and no historical runtime was executed.

| Change | Observed product/API semantics and source |
| --- | --- |
| [#370](https://github.com/loopflowstudio/loopflow/pull/370), `18da2d23a` — hardening | Removed `select: one` / `select: prompt`: forks run **all** branches, wait, write a manifest, and synthesize. Added prefixed logs, executor timeout (default 45 minutes), RAII scheduler slots, and orphan cleanup. See [`SchedulerSlotGuard`](https://github.com/loopflowstudio/loopflow/blob/18da2d23a6b3fe9770cc60960891ef2737b68f76/rust/loopflow/src/lfd/scheduler.rs) and [`recover_startup` / `cleanup_orphaned_fork_runs`](https://github.com/loopflowstudio/loopflow/blob/18da2d23a6b3fe9770cc60960891ef2737b68f76/rust/loopflow/src/lfd/executor/wave/mod.rs). |
| [#415](https://github.com/loopflowstudio/loopflow/pull/415), `b3a51739c` — workspace parity | Standardized sibling paths as `<parent-worktree>-fork-N`. `AgentRunContext` / `AgentLaunchRequest` carry explicit branch identity; Docker prefers it, then checked-out branch, then fallback. Path inference remains only in recovery. Manifest writes/removals and ephemeral cleanup moved behind executor workspace operations. See [Docker resolution](https://github.com/loopflowstudio/loopflow/blob/b3a51739c94403432dc1ece5a20a90ce04361081/rust/loopflow/src/lfd/executor/docker.rs) and [contemporaneous proof/limits](https://github.com/loopflowstudio/loopflow/blob/b3a51739c94403432dc1ece5a20a90ce04361081/scratch/remote-fork-executor-cleanup.md). |
| [#460](https://github.com/loopflowstudio/loopflow/pull/460), `cc7f92331` — multi-step branches | `fork: { flow: build, drafts: [...] }` or explicit `branches` mixing `flow:` and `step:` expanded to `ConcreteForkBranch { steps, label, directions, flow_parents }`. Branches ran concurrently; each branch ran steps sequentially and stopped at its first failure. Nested forks and interactive branch steps were rejected. See [expansion](https://github.com/loopflowstudio/loopflow/blob/cc7f92331977dd1d7ce2e1aef2b71ad777550c35/rust/loopflow/src/engine/flow.rs), [plan/manifest types](https://github.com/loopflowstudio/loopflow/blob/cc7f92331977dd1d7ce2e1aef2b71ad777550c35/rust/loopflow/src/engine/fork.rs), and [daemon execution](https://github.com/loopflowstudio/loopflow/blob/cc7f92331977dd1d7ce2e1aef2b71ad777550c35/rust/loopflow/src/lfd/executor/wave/fork.rs). |
| [#565](https://github.com/loopflowstudio/loopflow/pull/565), `2573c2ac2` — terminology | `fork` became `and` (parallel-all); `branch` became `or` (pick-one at this revision). The latter gained named routers, no-op silence paths, and CLI execution using `scratch/route-or.md`. This was selection before execution, distinct from comparing completed parallel results. See [usage diff](https://github.com/loopflowstudio/loopflow/commit/2573c2ac233ac588c11a48f898ac01ff49143c14) and [parser/types](https://github.com/loopflowstudio/loopflow/blob/2573c2ac233ac588c11a48f898ac01ff49143c14/rust/loopflow/src/engine/flow.rs). Do not assume these historical names describe current XOR semantics. |
| [#872](https://github.com/loopflowstudio/loopflow/pull/872), `309575f8e` — deletion | Removed `engine/fork.rs`, `ConcreteAnd` / `ConcreteAndBranch`, `SkillExecutor::run_and`, CLI parallel execution, and synthesis skill alongside the daemon/store architecture. The PR describes removing speculative runtime machinery; it does not establish that UX alternatives are unwanted. See [deletion](https://github.com/loopflowstudio/loopflow/commit/309575f8e) and [last pre-deletion CLI](https://github.com/loopflowstudio/loopflow/blob/be285d5d401fe150ac2f9bdfa884b5dbcc7f6366/rust/loopflow/src/lf/commands/flow.rs), which still supported a configurable synthesis skill. |

### Workspaces and result handling

At #460, CLI `run_fork` created `<current-branch>-fork-N` branches; the daemon
used `<run-id>-fork-N`. Base and branch directions were merged and deduplicated.
The CLI launched a thread per branch and invoked `lf <step> -b` by name inside
that worktree. The daemon acquired one scheduler slot for the branch's sequential
steps. See [`run_fork` / `run_fork_branch_steps`](https://github.com/loopflowstudio/loopflow/blob/cc7f92331977dd1d7ce2e1aef2b71ad777550c35/rust/loopflow/src/lf/commands/flow.rs).

`.lf/fork-manifest.json` held branch index, direction, worktree, git branch,
aggregate exit code, and each executed step's name/exit code. The daemon also
stored `ForkRun` identity, parent run/step, branch index, status, and worktree.
All branches were collected before synthesis, even if some failed. Missing
daemon results became failures. Synthesis ran in the parent workspace; afterward,
any failed branch still failed the fork. The runtime did not automatically merge
branch code. The [synthesis skill](https://github.com/loopflowstudio/loopflow/blob/cc7f92331977dd1d7ce2e1aef2b71ad777550c35/rust/loopflow/src/engine/builtins/steps/ops/synthesize.md)
asked for a unified `scratch/synthesis.md`, explicitly favoring combined insights
over choosing a winner. That contract did not promise runnable variants for later
human comparison.

### Costs and recovery implications

These are source observations and design implications, not measured incidents:

- **Retention:** both adapters cleaned manifest/worktrees after synthesis,
  including synthesis failure. [`cleanup_workspace_worktree`](https://github.com/loopflowstudio/loopflow/blob/cc7f92331977dd1d7ce2e1aef2b71ad777550c35/rust/loopflow/src/lfd/executor/mod.rs)
  called [`remove_worktree(..., true)`](https://github.com/loopflowstudio/loopflow/blob/cc7f92331977dd1d7ce2e1aef2b71ad777550c35/rust/loopflow/src/engine/worktree.rs),
  forcing worktree removal and attempting branch deletion. Useful alternatives
  therefore needed copying before cleanup; failure did not preserve them by contract.
- **Recovery:** #370's [orphan query](https://github.com/loopflowstudio/loopflow/blob/18da2d23a6b3fe9770cc60960891ef2737b68f76/rust/loopflow/src/lfd/store/sqlite.rs)
  selected Pending/Running forks whose parent was absent, inactive, or at another
  step. Startup removed their worktrees and records. Its test proves cleanup,
  not resuming an interrupted multi-step branch at its last successful step.
- **Identity and pinning:** index-based sibling paths and one manifest per parent
  workspace create a concurrency/revisit question for current loopflows. #460's
  CLI retained step names for subprocess dispatch, not captured skill bytes.
  Neither fact supplies current exact invocation/visit recovery semantics.
- **Cost:** N alternatives require N workspaces and the sum of their agent work,
  plus synthesis; the slowest branch gates collection. #370 added timeouts and
  slot guards; #415 repaired host/container divergence. Its note records slower
  sequential cleanup as a residual risk and two Docker startup tests blocked by
  a missing socket. No token, latency, or cost benchmark was recovered.

## Decisions for the follow-up

1. **Artifact variants or execution forks?** Can one skill produce separately
   runnable directories/routes with shared fixtures, or do independent approaches
   require separate Runs/worktrees? Compare the same UX question before choosing.
2. **Compare, select, or synthesize?** Preserve alternatives until the human can
   try them. Decide whether the result is one selected prototype, combined behavior,
   or unresolved tradeoffs; distinguish selection from production code merging.
3. **What repeats?** If parallelism earns a Flow primitive, define whether Iterate
   revisits one alternative or the whole section, how direction reaches branches,
   and when the parent continues. Fit the shared cursor, captured definitions,
   Advance/Iterate, Blocked/Ask, and exact human boundaries. Do not reintroduce
   pass limits or a Wave sequencer to host it.
4. **Who owns retained work?** Define stable variant identity, preview entrypoints,
   fake-data resets, partial failure/cancel/retry, process ownership, and cleanup.
   Artifact retention may be necessary even when execution worktrees are temporary.
   A failed branch must not erase successful comparisons or approve another gate.

## Done when

- A portable software UX prototyping skill has a bounded contract and produces
  runnable web/JS artifacts with hand-built fixtures, scenario/reset controls,
  launch instructions, and explicitly simulated effects.
- On one concrete software interaction, at least two structurally different
  alternatives can be exercised through the same meaningful state transitions.
  Record what trying them taught and the resulting implementation direction.
- A small experiment compares artifact variants with independent execution,
  recording setup, elapsed time, available usage, and retained outputs. Make an
  evidence-backed mechanism decision; choosing no new Flow primitive is valid.
- The chosen design specifies retention and recovery through partial failure,
  interruption, revisit, and human comparison. If forks are selected, provide a
  bounded integration plan and proof matrix for ordinary and Task-attributed
  loopflows, exact decisions, pinned definitions, and no duplicate branch launch.
  Runtime implementation requires that design; this research does not authorize it.

Review finding: rebuilding the historical always-synthesize-and-delete contract
would leave the motivating human comparison unresolved. Start with the runnable
experience and let its demonstrated isolation needs determine the machinery.

Only this note was written. No Task filing, code/skill edits, runtime tests,
installation, provider/review launch, delegation, commit, or publication occurred.
