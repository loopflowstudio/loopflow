# Rebase Efficiency: Placement, Classification, And Workflow Telemetry

## What to build

Make `lf op` the deterministic owner of branch/worktree placement and rebase
strategy so Loopflow avoids long, avoidable rebases without asking an agent to
reason about cases the system can classify algorithmically.

Jack's north star:

> we humans who use loopflow (which uses agents that use loopflow) dont waste
> time on long avoidable rebases. make the loopflow system smarter and then make
> the grooves that take you down the better paths easier to get to

## Product shape

The new center of gravity is **placement**:

```text
placement = resolve base + derive branch name + create/select worktree
```

Both explicit worktree commands and future normal `lf` execution placement flags
call the same engine:

```bash
lf op wt create api
lf op wt create --main api
lf op wt create --stack goals.m1 api

lf implement "add API tests" --stack goals.m1
lf implement "investigate CI" --fork
lf implement "quick isolated pass" --dispatch
```

`lf op wt create ...` creates or selects the placement. `lf <flow-or-step> ...
--stack|--fork|--dispatch` uses the same placement engine, then launches the
agent in that worktree. The implementation can land the `lf op wt` side first,
but the abstraction should be shaped for both surfaces from the start.

## Invariants

Branch names encode ancestry. Metadata may cache or validate, but must not be
required to understand stack shape.

```text
a       parent: main
a.b     parent: a
a.b.c   parent: a.b
```

Users do not manually type ancestry. They create ancestry through placement:

```bash
lf op wt create child              # stacked on current branch if not on main
lf op wt create --main child       # root branch from origin/main
lf op wt create --stack a.b child  # child of explicit parent
lf op wt create --fork child       # independent from current review base
```

Dots are reserved. User-provided worktree/branch segments cannot contain `.`.
If someone types `api.v2`, fail fast:

```text
"api.v2" is not a worktree segment. Dots are reserved for stack ancestry.
Use `api-v2`, or create ancestry with `--stack`.
```

Default base policy:

```text
current branch is main/default -> create root branch from origin/main
current branch is not main     -> create child branch from current branch
--main                         -> force origin/main
--stack <parent>               -> force explicit parent
--fork                         -> create independent branch from review base
```

The goals-wave architecture is binding here: `engine/worktrees` owns THE naming
rule. `lfd` does not own placement. Future `lfq` and lfd routes exec `lf`; they
do not reimplement this logic.

## Data structures

Add the branch-name and placement concepts under `engine/worktrees` or a sibling
`engine/placement` module that is still owned by `engine`.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeSegment(String);

impl WorktreeSegment {
    pub fn parse(raw: &str) -> Result<Self, PlacementError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackBranch {
    pub name: String,
    pub segments: Vec<WorktreeSegment>,
}

impl StackBranch {
    pub fn parse(branch: &str, default_branch: &str) -> Option<Self>;
    pub fn parent(&self) -> Option<String>;
    pub fn child(&self, segment: &WorktreeSegment) -> String;
    pub fn depth(&self) -> usize;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementRequest {
    Default { segment: WorktreeSegment },
    Main { segment: WorktreeSegment },
    Stack { parent: String, segment: WorktreeSegment },
    Fork { segment: WorktreeSegment },
    Dispatch { segment: Option<WorktreeSegment> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementPlan {
    pub base_ref: String,
    pub parent_branch: Option<String>,
    pub branch: String,
    pub worktree_path: PathBuf,
    pub stack_depth: usize,
    pub strategy: PlacementStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementStrategy {
    CreateRoot,
    CreateStackChild,
    CheckoutExisting,
    UseExistingWorktree,
}
```

Add a rebase classifier under `ops/rebase.rs` or `ops/rebase_plan.rs`. Keep the
actual git mutation separate from classification so tests can run without
expensive histories.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseClass {
    Current,
    StaleEmpty,
    ScratchOnly,
    GeneratedOnly,
    CleanAuthored,
    StackParentOpen,
    StackParentLanded,
    Protected,
    ConflictLikely,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseStrategy {
    Noop,
    ResetToBase,
    DirectRebase,
    RebaseOntoParent,
    SkipParentRebaseOntoMain,
    AgentRebase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebasePlan {
    pub branch: String,
    pub base_ref: String,
    pub stack_parent: Option<String>,
    pub class: RebaseClass,
    pub strategy: RebaseStrategy,
    pub unique_commits: usize,
    pub changed_files: Vec<PathBuf>,
    pub protected: bool,
    pub scratch_stash: Option<PathBuf>,
}
```

## Rebase policy

`lf op rebase` remains the user-facing command, but the verb means:

> make this branch current with the right base while preserving authored intent.

A git rebase is one strategy. Reset/recreate is the correct strategy when there
is no authored intent to preserve.

Aggressive deterministic reset cases:

```text
no unique commits
  -> reset to base

only checkpoint/generated commits and no protected paths
  -> reset to base

only scratch/ changes
  -> stash scratch, reset to base, restore scratch

scratch dirty on an otherwise disposable branch
  -> copy scratch aside, reset, copy scratch back
```

Protected cases:

```text
any wave/** change
any .lf/steps/**, .lf/flows/**, .lf/directions/**, .lf/config.yaml change
any code/test/docs change outside scratch
PR with comments/reviews/meaningful check history
child branch with meaningful diff
```

`scratch/**` is portable working context, not branch intent. Keep it simple:

```text
before reset/rebase:
  copy scratch/ to .lf/scratch-stash/<branch>-<timestamp>/

after reset/rebase:
  copy scratch/ back if doing so does not overwrite newer scratch
  if it would overwrite, keep the stash and print the path
```

No patch math in v1. No agent launches for scratch-only conflicts.

Stack-aware default:

```text
branch a.b.c, parent a.b exists locally or on origin
  -> rebase onto a.b

branch a.b.c, parent a.b landed/gone/squash-equivalent on main
  -> skip parent and rebase onto origin/main
```

## Observability

Add ops telemetry from the first implementation, not after behavior changes.
Write local JSONL under `.lf/metrics/ops.jsonl` or reuse the existing run journal
if that is less code. Do not include diffs, secrets, or raw prompt content.

Each `lf op wt create`, `lf op rebase`, `lf op land`, and later placed `lf`
execution emits:

```json
{
  "ts": "2026-07-05T00:00:00Z",
  "op": "rebase",
  "branch": "goals.m1.api",
  "base_ref": "goals.m1",
  "stack_parent": "goals.m1",
  "class": "scratch_only",
  "strategy": "reset_to_base",
  "unique_commits": 0,
  "changed_files": 1,
  "protected": false,
  "scratch_stashed": true,
  "agent_launched": false,
  "duration_ms": 421,
  "exit_status": "ok"
}
```

Also add human-readable status lines:

```text
Classified branch: scratch_only
Strategy: reset_to_base
Stashed scratch: .lf/scratch-stash/goals-m1-api-20260705T120102Z
Reset branch to origin/main.
Restored scratch/.
```

Add a dry-run/plan mode:

```bash
lf op rebase --plan
lf op wt create --plan --stack goals.m1 api
```

The plan output is deterministic and testable:

```text
branch: goals.m1.api
base: goals.m1
class: stack_parent_open
strategy: rebase_onto_parent
agent_launched: false
```

## Tests

Add unit tests around pure parsing and classification first, then shell E2Es
that exercise the real CLI in disposable git repositories.

### Unit tests

Placement:

- `WorktreeSegment::parse("api")` succeeds.
- `WorktreeSegment::parse("api.v2")` fails.
- `StackBranch::parse("a.b.c").parent() == Some("a.b")`.
- default placement from `main` creates root branch.
- default placement from `a.b` creates `a.b.child`.
- `--main` from `a.b` creates root branch.
- `--stack a.b child` creates `a.b.child` from anywhere.
- branch checked out in another worktree returns `UseExistingWorktree` or a
  clear "use that worktree" error, never silent cross-worktree mutation.

Rebase classifier:

- no unique commits -> `StaleEmpty` + `ResetToBase`.
- checkpoint-only commits touching only scratch -> `GeneratedOnly` +
  `ResetToBase`.
- dirty scratch only -> `ScratchOnly` + `ResetToBase` with stash.
- `wave/**` change -> `Protected`.
- `.lf/steps/**` change -> `Protected`.
- code/test/docs change -> `CleanAuthored` or `ConflictLikely`.
- branch `a.b.c` with parent `a.b` -> `StackParentOpen`.
- branch `a.b.c` with squash-merged parent -> `StackParentLanded`.

Telemetry:

- serialized op event has no raw diff.
- strategy/class names are stable strings.
- failed command records `exit_status: "error"` and error class.

### E2E tests

Create new shell tests under `tests/e2e/workflows/` or extend existing E2Es if
the project prefers a flat directory.

```bash
tests/e2e/test_rebase_stale_empty.sh
tests/e2e/test_rebase_scratch_stash.sh
tests/e2e/test_rebase_protected_wave.sh
tests/e2e/test_wt_stack_default.sh
tests/e2e/test_wt_reject_dot_segment.sh
tests/e2e/test_op_command_surface_parity.sh
```

Scenarios:

```text
stale empty branch
  main advances
  feature has no unique commits
  lf op rebase chooses reset_to_base
  no rebase agent launches

scratch stash
  scratch/design.md is dirty
  branch has no meaningful work
  lf op rebase stashes scratch, resets, restores scratch

protected wave
  branch changes wave/goals/MEMORY.md
  lf op rebase does not reset

stack default
  current branch is a.b
  lf op wt create c creates branch a.b.c and worktree ../repo.a.b.c

main escape
  current branch is a.b
  lf op wt create --main c creates root branch c/schema-root-c

dot rejection
  lf op wt create api.v2 fails with dots-reserved message

command surface parity
  generated loopflow prompt commands parse under the active lf binary
```

## Implementation sequence

### 1. Instrument before changing defaults

Add telemetry scaffolding and `--plan` output. Wire it into the current `rebase`
and `wt create` paths with existing behavior. This gives before/after data and
a safe place to test formatting.

### 2. Add placement planning

Introduce `WorktreeSegment`, `StackBranch`, `PlacementRequest`, and
`PlacementPlan`. Refactor `lf op wt create` to call `plan_placement` and
`apply_placement`.

Keep old `--stack` behavior working while adding:

```bash
lf op wt create --main <segment>
lf op wt create --plan ...
```

Flip the default to stacked-from-current when current branch is not main.

### 3. Add scratch stash

Implement simple directory-copy stash/restore around rebase/reset strategies.
Keep it intentionally dumb and visible.

### 4. Add rebase classification

Build `plan_rebase(repo, options)` and make `lf op rebase --plan` print it.
Then execute strategies:

```text
Noop
ResetToBase
DirectRebase
RebaseOntoParent
SkipParentRebaseOntoMain
AgentRebase
```

Only `AgentRebase` should reach the existing rebase-agent launch path.

### 5. Land/advance split as a measured experiment

Do not let this block the first demo, but shape telemetry for it now. Add an
explicit `lf op land --advance` or `lf op land --no-advance` plan after the
classifier lands. The current behavior can stay temporarily if changing it
explodes tests.

### 6. Prepare normal `lf` placement flags

Add the parser/API shape for `--stack`, `--fork`, and `--dispatch` on ordinary
`lf` runs only if it is cheap in this branch. If not, leave the placement module
ready and document the follow-up. The important thing is that future execution
flags call the same placement engine as `lf op wt create`.

## Demo

The demo should make the saved time visible.

```bash
# 1. Start from a stale branch with only scratch.
lf op rebase --plan
```

Expected:

```text
class: scratch_only
strategy: reset_to_base
agent_launched: false
```

Then:

```bash
lf op rebase
```

Expected:

```text
Stashed scratch/
Reset branch to origin/main
Restored scratch/
```

No rebase agent launches.

Next, show placement:

```bash
git checkout -b demo.parent
lf op wt create child
```

Expected:

```text
Created branch demo.parent.child
Created worktree ../loopflow.demo.parent.child
```

Then:

```bash
lf op wt create api.v2
```

Expected:

```text
error: dots are reserved for stack ancestry
```

Finally:

```bash
tail -n 5 .lf/metrics/ops.jsonl
```

Expected: visible decisions for the reset and placement operations, including
`strategy`, `stack_parent`, `agent_launched: false`, and duration.

This is the story: Loopflow saw a branch that used to burn a long rebase, made
the deterministic call, preserved the useful scratch, created a correctly
stacked child with one command, and left telemetry proving what happened.

## Done when

- `lf op rebase --plan` classifies stale-empty, scratch-only, protected, and
  stack-parent cases deterministically.
- `lf op rebase` resets disposable branches without launching an agent.
- `scratch/` survives reset/rebase through simple stash/restore.
- `lf op wt create` uses the placement planner.
- user segments containing `.` are rejected.
- branch-name ancestry works for `a.b.c -> a.b`.
- `lf op wt create child` from a non-main branch creates a stacked child, with
  `--main` as the escape hatch.
- ops telemetry records strategy decisions without raw diffs.
- E2E tests cover the demo path.
- `cargo fmt`, `cargo test -p loopflow`, and the relevant E2E workflow tests
  pass.

## Open choices to settle during implementation

- Exact generated/checkpoint commit subject allowlist.
- Whether root branch names keep the configured timestamp schema while stack
  children are literal dotted branches, or whether this branch introduces a new
  stack-aware schema formatter.
- Whether `lf op land` flips to no-advance in this branch or waits for a follow
  up after telemetry confirms the repair pain.
- Whether telemetry lives in `.lf/metrics/ops.jsonl` or the existing run journal
  fold. Prefer the smallest code path that can answer the metrics.
