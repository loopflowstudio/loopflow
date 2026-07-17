# Recommend only executable Task actions

## Intent

Make the existing six-action projection tell the truth about the newest Task PR.
This is not a new taxonomy for every historical gate or body failure. The bug is
smaller: snapshot builders erase `Merged` and `Abandoned` before
`derive_task_actions` sees them, then status recommends `resume` even though the
runner requires an active PR.

The repaired boundary has four parts:

1. Build action evidence from the newest PR, while keeping `active_pr` as the
   separate operational identity used by PR commands.
2. A newest abandoned PR recommends `start_next_pr`, never `resume`.
3. Status and `resume_task_async` evaluate one no-active-PR refusal before any
   process generation is reserved.
4. A newest merged PR receives the existing completion gate's refusal string;
   no new blocker enums or parallel gate evaluator are introduced.

## Computable contract

### User-visible outcome

A supervisor reading a settled Task sees a next action that matches the command
the lifecycle can execute. For W2-285, both human and JSON status name the
Project review gate instead of recommending Resume; an explicit Resume returns
the displayed no-active-PR refusal without creating a process generation. For
W2-286, the pending directive selects the existing serial `start_next_pr` path.
For an authoritatively abandoned latest PR, status recommends `start_next_pr`,
never Resume. W2-284's clear merged gate still recommends and completes the
Task.

### Source of truth and derived views

The authoritative lifecycle record is the Task Session's persisted `TaskPr`
history ordered by sequence, plus the persisted reviews and directive versions
read by `task_completion_gate`. The newest PR row decides the action phase.
Separately, the active PR row is the mutable identity used by PR operations,
fresh CI evidence, and stack-predecessor checks.

`TaskActionModel` remains a pure derived projection. `TaskSessionSnapshot` and
`TaskAttentionSnapshot` serialize that same model; clients do not recompute
legality. `CompletionGate` remains the completion authority, and
`no_active_pr_resume_refusal` becomes the shared Resume authority for status and
command execution.

### Affected surfaces and compatibility

- `lf task status` human output and `--json` consume
  `TaskSessionSnapshot.actions`.
- `lf status` and `lf roadmap` consume `TaskAttentionSnapshot.actions`; the Mac
  `WaveWorkMap`/`RoadmapView` decodes and acts on that same server projection.
- `lf task resume` consumes the shared Resume refusal before
  `resume_session`; `lf task complete` consumes the existing completion-gate
  refusal; `lf pr next` keeps the serial rotation authority.
- The wire shape and six action names do not change. Swift models, schema,
  migrations, and DTO fixtures remain compatible; only recommended values and
  reason strings change for settled PR evidence. Swift fixtures need changes
  only if they encode one of those corrected settled cases.

### End-to-end proof

Materialize a store-backed W2-285-shaped Task: newest PR #1037 is Merged,
`active_pr` is absent, and the required Project review blocks completion. Read
it through both snapshot builders and assert their `TaskActionModel` values
name Review, block Complete with the exact `task_complete` error, and block
Resume with the exact `task_resume` error. Invoke Resume and assert the stored
latest generation is byte-for-byte unchanged. Render `lf task status --json`
from the same snapshot to prove the external surface carries the model rather
than a test-only evidence value.

The W2-286 and W2-284 fixtures then prove the two other merged branches:
pending directive selects an executable serial successor, while an empty gate
still completes. A small builder-backed abandoned case proves real ordered PR
history reaches `PrPhase::Abandoned` and selects `start_next_pr`.

### Absent and error states

- No PR history is an invalid live-Task state, not a parked body. The shared
  helper returns `Task <id> has no active PR to resume; no PR history recorded`,
  Resume is unavailable, and the projection recommends NoAction rather than
  minting a doomed generation.
- A latest Working, Publishing, or Open PR is also the operational active PR and
  keeps the existing body/publication/CI behavior. Settled latest evidence may
  have no active PR by definition; that absence is the state being modeled,
  not missing evidence.
- A completion-gate store read failure propagates from the snapshot or command;
  neither surface guesses that completion is available. An empty blocker list
  is the only satisfied gate.
- Terminal Session and abandon-intent precedence remain unchanged: both select
  NoAction before PR routing.
- GitHub can degrade or change between status and a mutating command. The action
  model guarantees the durable/local preconditions covered here; the existing
  authoritative rotation check may still refuse a later `start_next_pr` on a
  degraded PR read. This Task does not convert external dependency failures
  into action-model state.

### Operational boundary

Refusing Resume performs no tmux launch, subprocess spawn, lease reservation,
or `ChildProcessGeneration` write. Snapshot construction adds only local store
reads already available to the two builders; it performs no new network call
and no mutation. The W2-300 range calculation remains single-evaluation during
an invoked rotation, and status never computes the git range. Publication stays
manual through `lf pr publish`; this Task never arms auto-merge.

## Evidence

`task_prs` is ordered by sequence. Both snapshot builders nevertheless select
only:

```rust
let active = prs.iter().find(|pr| pr.is_active());
```

`PrPhase::is_active()` is `Working | Publishing | Open`, so the evidence sent to
`derive_task_actions` can never contain `Merged` or `Abandoned`. The model's
existing settled-PR arms are reachable only from hand-built tests. Real merged
and abandoned Tasks instead fall through `active_pr_phase: None` to
`body_model`, which recommends `resume` and prints `"implementation not
finished"`.

W2-285 proves the recommendation is operationally false. PR #1037 had merged,
status recommended `resume`, the command was accepted, generation 3 was
created, and the runner then failed with `Task Session ... has no active PR`.
W2-286 had the same projection after merged PR #1032, while its real completion
blocker was unincorporated directive v2. W2-284 is the successful control: its
merged PR completed while the body was still live.

The completion gate itself is already correct. `task_completion_gate` checks
required reviews, pending directives, and unsettled PRs, and `task_complete`
renders `CompletionGate::reason()`. The action model needs that value; it does
not need a second representation of the blockers.

## Production shape

### Newest PR drives the projection

In both `ops/task.rs::task_snapshot` and
`lf/commands/waves.rs::build_task_detail`, derive two independent values from
the same ordered history:

```rust
let latest = prs.last();
let active = prs.iter().find(|pr| pr.is_active());
```

`active_pr` remains `active.map(|pr| pr.id.clone())` and continues to identify
the PR that operational commands may mutate. Only `TaskActionEvidence` switches
from `active` to `latest` for phase/disposition routing:

```rust
pub struct TaskActionEvidence<'a> {
    pub status: TaskSessionStatus,
    pub latest_pr_phase: Option<PrPhase>,
    pub latest_pr_after_merge: Option<AfterMerge>,
    pub latest_pr_next_slug: Option<&'a str>,
    pub completion_refusal: Option<&'a str>,
    pub resume_refusal: Option<&'a str>,
    pub pending_directive: bool,
    // existing CI, process, predecessor, review, abandon, and progress evidence
}
```

The three current `active_pr_*` evidence fields become `latest_pr_*`; there is
no `SettledPr` wrapper. CI still comes only from `active.and_then(fresh_ci)`.
Predecessor evidence still comes from the active PR because stack blocking is an
operational property of the branch currently being worked.

This makes `Merged` and `Abandoned` reachable through the real builders and
removes those phases from the exhaustive test's synthetic-only state space.

### One resume refusal, before generation reservation

Add one helper beside Task operations:

```rust
pub(crate) fn no_active_pr_resume_refusal(
    identifier: &str,
    active: Option<&TaskPr>,
    latest: Option<&TaskPr>,
) -> Option<String>;
```

It returns `None` when an active PR exists. With no active PR it returns one
actionable sentence, enriched by the newest PR when present, for example:

```text
Task W2-285 has no active PR to resume; pull request #1037 merged
```

The snapshot builders place that exact `String` in
`TaskActionEvidence::resume_refusal`; `derive_task_actions` uses it for the
blocked Resume status. `resume_task_async` re-reads the PR history after
`reconcile_task_pr`, calls the same helper, and returns the same string before
`resume_session`. The runner's active-PR check remains as a defensive invariant.

The command-side order is:

1. reconcile authoritative PR state;
2. calculate newest and active PR;
3. return the shared refusal, if any;
4. only then run liveness recovery and `resume_session`.

No refused resume can reserve a `ChildProcessGeneration`.

### Existing completion gate feeds the merged model

Keep `CompletionGate { satisfied, blockers: Vec<String> }`. Add only a formatter
that preserves the command's current wording:

```rust
impl CompletionGate {
    fn refusal(&self, identifier: &str) -> Option<String> {
        (!self.satisfied).then(|| format!(
            "Task {identifier} cannot complete until its gates close: {}",
            self.reason(),
        ))
    }
}
```

`task_complete` returns this value. Each snapshot builder evaluates
`task_completion_gate` once and lends the same value to
`TaskActionEvidence::completion_refusal`. The merged model therefore blocks
Complete with the command's exact refusal rather than `"implementation not
finished"`.

No `CompletionBlocker`, `CompletionEvidence`, review identity DTO, or second
gate query is added.

### Settled action selection

For a newest abandoned PR, `derive_task_actions` recommends `StartNextPr`.
`ensure_working_pr_with_authority` already rotates from an authoritatively
closed abandoned predecessor, so this recommendation names the command that can
run. Resume is blocked by the shared refusal.

For a newest merged PR:

- no completion refusal: preserve the existing `AfterMerge` behavior;
- required-review refusal: recommend `Review` and put the existing gate refusal
  on Complete; Resume remains blocked by the shared no-active-PR refusal;
- pending-directive refusal: recommend `StartNextPr`, carrying the accepted
  direction into the existing serial PR path rather than inventing an
  acknowledgement bypass.

The last case composes with W2-300's committed-follow-up rotation fix. W2-286's
PR #1032 records `after_merge: CompleteTask`; a pending directive must authorize
the same serial successor that W2-300 authorizes when committed work exists
after the merged head. After rebasing, preserve W2-300's single
`committed_follow_up_range` calculation before the completing-PR bar and use its
result both in the bar and in the later cherry-pick. Do not replace it or call
the range function a second time.

The exact combined refusal condition is:

```rust
let committed_follow_up = committed_follow_up_range(&session.worktree, &settled)?;

if settled_is_completing
    && !has_pending_directive(session)
    && committed_follow_up.is_none()
{
    return Ok(None);
}
```

Pending direction or committed follow-up independently authorizes the existing
serial successor. Only a completing PR with neither may settle through the
merge-to-completion path. The action model receives the existing
`has_pending_directive(session)` boolean only to choose the owner; W2-300's
committed-range evidence stays command-side, and the gate's reason remains the
single refusal value.

## Behavioral proof

Keep three incident fixtures plus one small abandoned-state unit case:

1. **W2-285 — merged + required-review blocker.** The newest PR is Merged,
   `active_pr` is absent, Complete shows the byte-identical
   `task_complete` refusal naming the required Project review, Resume shows the
   shared no-active-PR refusal, and no body generation is created.
2. **W2-286 — merged + pending directive.** The newest PR is Merged,
   `active_pr` is absent, Complete shows the byte-identical directive-v2 gate
   refusal, and `StartNextPr` is recommended and passes the serial rotation
   precondition because pending direction exists.
3. **W2-284 — merged + clear gate.** Complete remains recommended and succeeds;
   the successful path does not get parked.
4. **Abandoned route.** Evidence built from a real ordered PR history reaches
   the Abandoned arm, recommends `StartNextPr`, and never recommends Resume.

For every command-bearing action exercised by these fixtures, the test invokes
the corresponding command path: a recommendation succeeds, while a blocked
command refuses with the exact status reason. The refused-resume test compares
the latest generation before and after the call, not merely the returned error.

Sabotage checks:

- change the snapshot builder back to `find(is_active)`; the three merged
  fixtures must fail;
- make `no_active_pr_resume_refusal` return `None`; the no-generation test must
  fail;
- restore the unconditional `CompleteTask` rotation bar; the W2-286 executable
  recommendation test must fail;
- format Complete's status reason independently; the status/command equality
  assertion must fail.

## Scope

In scope:

- `task/actions.rs`: newest-PR evidence names, settled routing, borrowed
  completion/resume refusals, and action-model tests;
- `ops/task.rs`: newest/active snapshot selection, shared resume refusal,
  existing-gate formatter, pre-generation resume bar, and the pending-directive
  composition with W2-300's single committed-follow-up rotation check;
- `lf/commands/waves.rs`: feed newest PR and the two shared refusals into the
  second action-evidence builder;
- behavioral fixtures proving W2-285, W2-286, W2-284, and the abandoned arm.

### Exclusions

- new completion-blocker enums or a second completion evaluator;
- status DTO additions or review identity fields;
- recover-liveness changes;
- moving or deduplicating `review_gate_from`;
- weakening either completion gate;
- auto-merge (`lf pr publish` remains the publication boundary).

## Estimate

Three production files, roughly 70–100 modified lines net:

- `task/actions.rs`: 25–35 lines across field renames and settled routing;
- `ops/task.rs`: 35–50 lines for the shared refusals, newest-PR builder, resume
  bar, and the pending-directive composition with W2-300's rotation condition;
- `lf/commands/waves.rs`: 10–15 lines to feed the same evidence.

Tests are the larger half: roughly 150–220 lines, mostly store-backed fixtures
that prove command execution/refusal and generation-count stability. No schema,
DTO fixture, Swift, runner, or migration changes.

## Done when

- Real snapshot builders can emit Merged and Abandoned action evidence.
- A settled Task never recommends Resume, and `lf task resume` refuses before
  creating a generation with the same reason status reports.
- W2-285 names its Project review gate; W2-286 names directive v2 and can enter
  the serial next-PR path; W2-284 still completes.
- `"implementation not finished"` is never emitted for a merged PR.
- The existing completion gate remains authoritative and unweakened.
- `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test -p loopflow` pass
  to completion.
