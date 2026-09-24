# Iteration 11 — truthful Active presentation, 2026-09-23

Active retains the human's requested membership: an open Session or an owned
live provider in the Task checkout. Provider in checkout replaces Running in
the row, and the derived property now names that location evidence. No Run/Task
attribution, lifecycle policy, shared DTO or activity authority changed.

The previous review's proposed Task-attribution restriction conflicted with this
explicit direction in `task-views.md`; it is superseded. Its second finding was
reproduced: a successful untruncated planning read with recorded out-of-plan
Tasks, no planned matches and successful empty Session/activity reads hid the
Wave and claimed No active Tasks. The navigator now uses one completeness check
for Wave visibility, count certainty and confirmed emptiness. Diagnostics stay
in Wave details, reachable through the retained heading.

## Proof

The new parameterized regression checks both incomplete association and a truly
empty plan. It clicks the retained heading and reads the recorded Task in details.
Before correction it fails on both the false empty message and missing heading:
`/tmp/loo291-iteration11-before.log` (exit 1).

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'WorkspaceNavigationTests/(activeEmptyRequiresCompletePlanning|workDoesNotRequireSessions|unavailableProjectLivesInWaveDetails)'
```

Three tests/four cases pass, exit 0: `/tmp/loo291-iteration11-after.log`.
No executable source changed afterward. No broad gate ran. The existing Active
query still passes Session and checkout membership checks, including closed
Sessions; the existing diagnostic relocation check also passes.

Review: kept planning evidence distinct from read failures; removed the duplicate
completeness decisions that allowed visibility and counts to disagree. Session
joins remain typed Work joins. No provider was attributed by its checkout, and
no new reader, persistent state, terminal owner or Session action was introduced.

The ongoing human demo was left untouched: no app launch, foreground interaction,
provider launch, transfer, completion or installed binary change. This is local
view/model evidence, not configured-provider or external-work proof. Configured
Session-row/nested interaction, shared LOO-284 integration, directive editing,
human-selected external trials and published performance budgets remain open.
No publication, landing or Task completion occurred.
