# Operations

`ops` holds the commands' work: Tasks, worktrees, PRs, Graph runs and planning
synchronization. Most modules explain themselves; this file documents the one
contract that spans processes and a shared request budget.

## Planning export

`planning_export.rs` gives every locally saved Task and Project its Linear (or
Asana, Notion) identity. Creation is idempotent by construction: the receipt in
`task_creation_intents` / `project_transitions` holds the generated id and the
exact input, a pass looks for that id before it ever sends a create, and a
re-sent create carries the same id, which Linear accepts at most once.

```text
lf task create            prepare receipt → look (FindExportIssue) → create (DeliverTaskCreation)
any lf command (a pass)   for each pending export: skip inside the window, else look, else re-send
lf task sync ISSUE        look now, ahead of the window, and re-send if absent
```

**Pass cost.** A pass over N pending exports costs no requests when nothing
changed. The repository's Team claim is validated once per process
(`ops/linear.rs`, `VALIDATED_TEAM_CLAIMS`), so `ListTeams` is sent at most once
per process. Queued-response fixtures (`linear::test_server::spawn`) therefore
answer one Team validation per repository, however many lookups follow. An attempted creation whose outcome is unknown (the reply was lost
or the deadline fired) is *uncertain*; its receipt records when a pass last
looked for it (`export_observed_at`), and the pass refuses it inside the window
before any client is built. An unattempted export is new work and always
proceeds; a mapped one is finished.

**Retry policy.** Only looks at uncertain creations are stamped, so a creation
whose first send timed out is looked for on the very next pass. If the look
finds the issue, the pass binds it (no second send). If it finds nothing, the
pass re-sends the creation with its saved id and logs that it did. A creation
that stays uncertain after that is looked for once per window: ten minutes while
the Linear budget is healthy, one pacing interval while it is paced, and not
until reset inside the reserve (`max(600, delta_fresh_secs())`). A pass logs
"looked Ns ago, next look in Ms" inside the window; `lf task sync ISSUE` is the
person's own look and ignores it.

**Deadline.** Each export has its own five-second deadline covering only the
provider path: Team validation, the look, the workflow state and the send. No
scope freshness check runs inside a write (`LinearClient::graphql` sends what it
is given); freshness belongs to work boundaries (`linear_observe::sync_scope`,
`team_current`) and to `deliver_task_state`, which calls `check_before_write`
itself. A timeout names the step it interrupted: "Export timed out while sending
the creation; creation identity and uncertainty retained".

Request accounting lives in `logs/linear-requests.jsonl` when
`logs.linear_requests` is on (`lf history linear`).
