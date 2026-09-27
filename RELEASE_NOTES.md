# v0.12.23

v0.12.23 makes unattended Tasks easier to steer and recover: agent choices persist, failed decisions expose their blockers, and stalled work becomes visible. Planning moves into Task, Wave, and repository commands, with retryable deletion that preserves historical evidence. Release retries can also reuse a version after its earlier release PR was withdrawn.

## Keep Task choices through execution and recovery

A Task's selected agent now survives launch, resume, restart, and review, overriding skill defaults without changing the configured default. The implementation loop is shorter, and CLI and desktop show blocked or stalled execution with recovery guidance.

- Failed decisions retain their Run and open a keyed unblock Session instead of leaving the Task looking ready.
- Five minutes without events or sampled body/tool CPU progress shows **Stalled**. Missing or mismatched samples remain **Unknown**; observing a stall does not authorize termination.
- The `pursue` Flow runs implement → compress → review-slice → loop-decide. Compress edits code; review-slice reports replacement evidence and convergence blockers. Concept review remains interactive, and shipping removes the redundant task-gate wrapper.

## Manage planning and deletion through one command family

Task creation, editing, comments, and completion now live under `lf task`. Deletion handles both registered and planning-only Tasks, so removing an issue no longer requires separate Linear and local actions.

- Use `lf task delete ISSUE` to delete a Task. Confirmed deletion removes it from current planning views while preserving completed outcomes, timestamps, authored files, and retained PRs. Explicit `lf task status ISSUE` keeps historical lookup available.
- Saved issue identity and deletion confirmation let retries survive planning refresh and chapter replacement. Missing provider data never counts as confirmed deletion. Chapter rotation now sends untouched backlog to Linear's native trash.
- Planning-only creation requires no checkout or execution credentials. `create --run` validates execution before filing the issue and reuses that issue on retry.
- Connection, sync, and placement move to `lf wave`; provider administration moves to `lf repo`; planning diagnostics use `lf doctor --planning`.
- Checkout Task identity comes from the checked-out branch, and completion evidence survives provider writeback retries.

## Operational notes

- **Command migration:** `pm`, `work`, and superseded Task commands are removed without aliases. Update scripts to the Task, Wave, repository, and doctor commands above; desktop callers, bundled skills, and documentation have been updated.
- Task deletion does not certify process termination or complete Session/Run cleanup. Recorded Runs retain their attribution.
- `lf release run` ignores release PRs closed without merging when finding a version's PR. A withdrawn PR no longer forces skipping that version; open and merged PRs still participate in lookup.
- The Swift CI test step now times out after 20 minutes, allowing a hung step to fail and be rerun. The intermittent observation-cancellation test hang itself remains unfixed.
- Only free disk space gates builds. Oversized sibling checkouts warn; the current checkout cleans its own oversized disposable build roots.
- Recorded checks cover the affected Task suites, deletion and retry behavior, and all 41 release-module tests. Configured Claude launch/resume, a live five-minute stall, rendered Task states, installed deletion acceptance, and end-to-end release retry remain unverified in the supplied evidence. The final decision-policy revision also lacks live proof; earlier policy evidence used simulated planning state and synthetic feedback.

## Small changes

- Ruff is updated from 0.16.3 to 0.16.4.
