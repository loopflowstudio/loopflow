# Research: Rebase And Worktree Efficiency

## System Understanding

Loopflow already has most of the machinery needed to make rebasing and branch
workflow measurable: `lf op` wraps git operations, worktree paths are
schema-generated siblings, E2E tests create disposable repos, and `ops::trace`
can emit JSON traces for parity testing.

The current pain is not one missing command. It is that branch/worktree/PR
operations are split across conventions, prompts, installed binaries, and
runtime paths. The same workflow can look unified in source while a live agent
sees a different command surface.

### Architecture

- `rust/loopflow/src/lf/mod.rs` defines the CLI surface. Source currently has
  `lf q worker run`, `lf op wt create --stack`, `lf op rebase`, `lf op land`,
  `lf op next`, `lf op queue reconcile`, and related branch/PR operations.
- `rust/loopflow/src/lf/commands/ops/mod.rs` adapts CLI commands to ops logic.
  Conflict paths in `rebase_current`, `land_current`, and `open_pr` all launch
  the `rebase` step agent after `OpsError::RebaseConflict`.
- `rust/loopflow/src/ops/rebase.rs` performs fetch/sync, detects one
  squash-merged stacked-parent case, then calls `git rebase`. Any remaining
  non-success becomes `RebaseConflict`.
- `rust/loopflow/src/ops/land.rs` always prepares, rebases, clears scratch,
  merges/enables auto-merge, then rotates/preserves recognized wave worktrees.
- `rust/loopflow/src/engine/worktrees.rs` is the canonical sibling-worktree
  path layer. It recognizes only sibling worktrees named `<repo>.<short-name>`;
  `.claude/worktrees` style paths intentionally do not resolve as waves.
- `tests/e2e/*.sh` already create disposable bare origins and local clones. The
  shape is right for scenario tests that exercise the real CLI without touching
  the developer repo.

### Data Flow

For a normal land:

```text
lf op land
  resolve current/main repo
  commit dirty work if needed
  rebase current branch onto origin/main
  create/update PR or local-merge
  clear scratch/
  rotate recognized wave worktree
```

For a normal worktree create:

```text
lf op wt create NAME
  sync default branch
  if origin/NAME exists: check it out
  else format a schema branch from NAME
  create sibling ../<repo>.<short-name>
  push branch in background
```

For rebase conflict handling:

```text
lf op rebase
  try direct rebase
  if conflict: return OpsError::RebaseConflict
  CLI catches it and launches the rebase agent
```

The missing layer is a preflight classifier before "try direct rebase" and
before "launch the rebase agent."

### Key Abstractions

Branch names should encode ancestry. If `a.b.c` exists, its parent is `a.b`.
Metadata can cache or validate, but humans and agents should not need metadata
to understand ancestry.

That gives a simple stack model:

```text
main
  a
    a.b
      a.b.c
```

Useful derived functions:

```rust
fn stack_parent(branch: &str) -> Option<&str>;
fn stack_root(branch: &str) -> &str;
fn stack_depth(branch: &str) -> usize;
fn child_branch(parent: &str, child: &str) -> String;
fn is_main_branch(branch: &str, default_branch: &str) -> bool;
```

## Tensions

- **Source CLI versus installed CLI**: source and goldens mention `lf q worker
  run`, but the installed `lf` in this worktree treats `q` as a step and fails.
  Dispatch drift is itself a measurable workflow bug.
- **Short commands versus uniform commands**: older workflow optimized for
  humans manually navigating worktrees. Agent-driven workflow benefits more
  from deterministic, verbose, introspectable behavior.
- **Automatic recovery versus agent recovery**: current `lf op rebase` escalates
  to an agent after any unresolved rebase failure. Many cases can be classified
  as reset, direct rebase, skip-parent rebase, or abandon/recreate before
  involving an agent.
- **Land rotation versus repairability**: `lf op land` currently mutates the
  worktree lifecycle immediately after land. That is efficient when everything
  works, but makes post-land repair and inspection harder.
- **Branch names as truth versus rich state**: branch names should encode the
  stack. State files can speed lookups and store telemetry, but must not become
  the required source of ancestry.

## Observations

### Adjacent Goals Work

The in-progress `jack-heart.goals.20260705_1100` worktree has no committed diff
from main, only scratch notes. It is a live example of the branch class this
design should handle cheaply: stale or branch-local scratch context without
committed product changes.

That scratch sketches a related M1 architecture direction: remove `lf q worker
run` as the public worker API and express placement as flags on ordinary `lf`
runs:

```bash
lf implement "task" --dispatch
lf implement "task" --stack X
lf implement "task" --fork
```

The useful alignment for this design:

- placement should be an execution concern of normal `lf`, not a separate
  detached worker command.
- worktree naming should have one owner under `engine/worktrees`.
- `lfd` should stop owning dispatch/worktree placement and become a
  gatekeeper/query surface.
- `.claude/worktrees/agent-*` exists in the current repo and is exactly the
  noncanonical sprawl `lf op` should make unnecessary.
- the goals architecture says `engine` owns worktrees plus "THE naming rule";
  this design should put branch ancestry parsing and dotted-name validation
  there, not in `ops` or `lfd`.
- channels already reserve dots as tree structure: "names are topics, dots are
  the tree, subscription by prefix." Branch/worktree ancestry can reuse the
  same mental model if user-provided segments cannot contain dots.
- `lfd` hand routes are target-state debt: routes like `/land`, `/next`, and
  `/stop` should exec `lf` rather than mutate git/tmux in process. Rebase and
  worktree strategy should therefore live behind `lf op` and be reusable by
  future `lfq`.
- `--pool` is not just an implementation detail. `wave-agent-follow-ups.md`
  calls it a shared-worktree/shared-branch collision point and says isolated
  branches plus stacking are supposed to replace it. This supports making
  stack/fork/dispatch placement the better groove.

The main conflict to resolve explicitly: the goals scratch says branch names
should not be the source of truth for stack structure, while this design's user
constraint is that branch names do fully encode ancestry (`a.b.c` has parent
`a.b`). For rebase-efficiency, branch-name ancestry wins; metadata may annotate
or validate but cannot be required to understand the stack.

### External Patterns

Graphite is the closest comparison for stacked branch mechanics. Its docs frame
branch dependencies as the thing vanilla git lacks, and `gt restack` as the
operation that brings every branch back onto the current version of its parent:
https://graphite.com/docs/restack-branches. It also treats branches as atomic
changesets, usually one commit per branch, which keeps stack reasoning cheap:
https://graphite.com/docs/create-stack.

Graphite's multi-worktree behavior is especially relevant for agents. It avoids
modifying branches checked out in another worktree, shows worktree paths in its
stack log, and tells users to run stack operations from each relevant worktree
when a stack is split across worktrees:
https://graphite.com/docs/multiple-worktrees. Loopflow should borrow the
invariant, not necessarily the UX: an `lf op` command should know when a child
branch is checked out elsewhere and classify that as "needs that worktree" rather
than silently crossing the boundary.

HumanLayer's public product material emphasizes tasks as the grouping unit for
agent sessions, artifacts, and worktrees. The visible task-creation flow includes
explicit worktree timing (`Now`, `Later`, `Never`), target worktree path, branch
name, multi-repo workspace setup, and per-phase artifacts:
https://www.humanlayer.dev/. The useful pattern is not their exact naming; it is
that worktree creation is a first-class task lifecycle decision with visible
state, not an incidental side effect hidden inside an agent.

Recent agentic-PR research reinforces that merge outcome alone is the wrong
metric. One 2026 study found rejected PRs often reflect workflow constraints or
missing rationale rather than agent failure, and another positions Cursor/Devin/
Copilot-style tools as agent-initiated while humans retain merge governance.
That supports measuring interaction events and workflow decisions, not just
"merged" versus "failed."

### Complexity

`land` is the densest path because it does commit, rebase, PR copy generation,
scratch cleanup, merge/auto-merge, URL opening, and worktree rotation. It is
also the place where rebase failure, PR failure, and rotation failure collapse
into one user workflow.

`rebase_with_recovery` is intentionally small, which is good, but currently has
only one special case: squash-merged parent detection. A classifier can remain
small if it returns explicit decisions rather than growing recovery behavior
inline.

### Quality

The E2E test harness is promising: it creates temporary remotes and clones and
can model realistic git histories. Current coverage is too narrow:

- `test_smoke.sh`: prompt assembly, basic worktree create, commit.
- `test_full_cycle.sh`: local land returns to main and deletes feature branch.
- `test_rebase_conflict.sh`: direct conflict returns non-zero.

Missing workflow scenarios:

- stale empty branch should reset/recreate without launching an agent.
- stale branch with clean unique commits should direct-rebase and push.
- branch with only generated checkpoint commits should be eligible for reset.
- branch with meaningful commits and textual conflict should escalate.
- stacked branch `a.b.c` should infer parent `a.b`.
- `lf op wt create child` from `a.b` should produce `a.b.child` by default if
  stacked-by-default wins, while `--main` should force root-from-main behavior.
- `lf op land` should be tested separately from worktree advancement.
- installed command-surface parity should verify generated prompts mention
  commands the active binary actually supports.

### Potential

`ops::trace` can become the backbone of cheap workflow parity tests. Add trace
events to `rebase`, `land`, `next`, and `wt create`, then compare decisions
without needing GitHub or real agent launches.

Disposable git repos can become a scenario suite:

```bash
tests/e2e/workflows/stale_empty_branch.sh
tests/e2e/workflows/stack_child_create.sh
tests/e2e/workflows/rebase_classifier.sh
tests/e2e/workflows/land_without_advance.sh
tests/e2e/workflows/command_surface_parity.sh
```

## Measurement Plan

### Workflow Outcome Metrics

Track these per `lf op` invocation:

- `op`: `wt.create`, `rebase`, `land`, `next`, `pr`, `queue.reconcile`.
- `branch`: current branch.
- `stack_parent`: derived from branch name, if any.
- `decision`: `reset`, `direct_rebase`, `skip_parent_rebase`,
  `agent_rebase`, `abandon_recreate`, `land_only`, `advance`.
- `dirty`: whether uncommitted changes existed.
- `unique_commits`: count against base/parent.
- `changed_files`: count against base/parent.
- `conflict_files`: count when rebase fails.
- `agent_launched`: boolean.
- `duration_ms`.
- `exit_status`.

The key product metrics:

- **Agent rebase rate**: percentage of rebases that launch an agent.
- **Avoidable rebase-agent rate**: stale/empty/generated-only branches that
  still launch an agent.
- **Median time from `lf op land` start to ready/queued/merged state**.
- **Post-land repair rate**: land operations followed by CI-fix/rebase on the
  same branch.
- **Command drift rate**: prompt-recommended commands that the installed `lf`
  cannot parse.
- **Conflict recurrence**: same file conflicts across branches in one wave.

### Scenario Tests

Start with deterministic shell E2Es. Each should assert the chosen strategy, not
just success/failure.

```text
stale empty branch
  main advances
  feature has no unique commits
  lf op rebase chooses reset/recreate, not agent_rebase

generated-only branch
  branch contains only checkpoint/generated commits
  main advances
  lf op rebase chooses reset or abandon_recreate when clean enough

meaningful clean branch
  branch has one unique non-conflicting commit
  main advances elsewhere
  lf op rebase chooses direct_rebase

semantic conflict branch
  branch and main edit same line
  lf op rebase chooses agent_rebase

stack child creation
  current branch is a.b
  lf op wt create c creates branch a.b.c and worktree ../repo.a.b.c
  lf op wt create --main c creates root branch c or schema-root equivalent

stack parent rebase
  current branch is a.b.c
  parent is a.b
  rebase target is parent when parent is unmerged, origin/main when parent landed

land without advance
  lf op land leaves current repairable worktree in place
  lf op next or lf op land --advance performs rotation
```

### Synthetic Workload

Add a replay harness that generates 50-100 disposable git histories:

- percentage stale-empty branches
- percentage one-file clean branches
- percentage mechanical conflicts
- percentage true semantic conflicts
- stack depth distribution
- parent landed versus parent open

Run current strategy and proposed classifier in trace mode. Compare:

- agent launches avoided
- wrong resets prevented
- direct-rebase success rate
- total commands run
- wall time

This gives a way to tune "practicality wins" thresholds without arguing from
anecdotes.

### Dogfood Loop

For two weeks, emit local JSONL telemetry under `.lf/metrics/ops.jsonl` or the
existing journal layer. Do not include raw diffs or secret values.

Review weekly:

- top conflict files
- branches reset versus rebased
- rebase-agent launches by decision reason
- failed `lf op` commands and parse errors
- land operations that needed follow-up repair

Then change exactly one default at a time:

1. stack-by-default `wt create`, with `--main` escape hatch.
2. stale-empty reset before rebase.
3. land/advance split.
4. generated-only branch reset policy.

## Open Questions

- What exact commit-message patterns count as generated/checkpoint-only?
- Should root branches still use timestamp schema while stack children use
  literal dot suffixes?
- Should `a.b.c` be legal if `a.b` does not exist locally but exists on origin?
- How should branch names escape dots inside user-provided child names?
- Should `lf op land` default to no-advance, with `--advance` restoring old
  behavior, or keep old behavior under `land` and add `land --no-advance`?

## Recommendations

### Add A Rebase Classifier Before Agent Escalation

**Observation**: `rebase_with_recovery` currently attempts one direct rebase
then escalates. Stale empty branches and generated-only branches can be handled
without an agent.

**Cost**: Medium. Requires branch diff classification, trace output, and E2E
coverage.

**Benefit**: High. Directly targets the most wasteful failure mode.

**Verdict**: Worth doing first.

### Make Stack Ancestry A Pure Branch-Name Function

**Observation**: The user wants `a.b.c` to fully encode ancestry. That keeps
agent reasoning simple and avoids hidden state drift.

**Cost**: Low to medium. Naming validation and escaping rules need care.

**Benefit**: High. This gives worktree creation, PR bases, and rebase targets a
single rule.

**Verdict**: Adopt as the core invariant.

### Split Land From Advancement

**Observation**: Current land rotates recognized wave worktrees immediately.
That optimizes a fast path but makes failed or post-land repair harder.

**Cost**: Medium because existing tests and prompts expect rotation.

**Benefit**: Medium to high. Cleaner lifecycle and easier recovery.

**Verdict**: Test both defaults in trace/dogfood mode, then choose the simpler
system behavior.

### Treat Command-Surface Drift As A First-Class Test

**Observation**: This checkout's source and prompts mention `lf q worker run`,
but the active installed binary does not support it.

**Cost**: Low. Add a parity test that compares generated loopflow guidance
against `lf --help`/parse results for listed commands.

**Benefit**: High for agent reliability. A prompt that tells agents to run
missing commands guarantees drift.

**Verdict**: Add early. It prevents future workflow instructions from becoming
fiction.
