# Operations

`ops` holds the commands' work: Tasks, worktrees, PRs, Graph runs and planning
synchronization. Most modules explain themselves; this file documents the one
contract that spans processes and a shared request budget.

## Tracker queue

Every save a tracker carries leaves one `tracker_actions` row (storage
`sqlite/tracker_actions.rs`); `tracker/drain.rs` is the only code that sends
them, through the repository's tracker in `tracker.rs`.

```text
any save                  queue the action → one pass, the saved owner first
lf repo tracker           list unconfirmed actions: owner, kind, phase, error
lf task sync ISSUE        drain that owner now, looking for lost replies at once
```

**Pass cost.** A pass sends at most 20 requests or runs 5 s, four owners at a
time. With nothing unconfirmed it sends none and builds no client. Only a queued
`create` builds the Team-validated Linear client; the repository's Team claim
is validated once per process (`ops/linear.rs`, `VALIDATED_TEAM_CLAIMS`), so
queued-response fixtures (`linear::test_server::spawn`) answer one Team
validation per repository, however many sends follow.

**Lost replies.** A send whose reply never came leaves its action `sent`.
Before anything else on that subject goes, a pass looks for it in the tracker,
at most once per `max(600, delta_fresh_secs())` seconds (`looked_at`): ten
minutes while the budget is healthy, one pacing interval while it is paced.
Linear comments and every link are sent again instead, since repeating them
cannot duplicate. `lf task sync ISSUE` ignores the window.

**Deadline.** Each request has its own five-second deadline; the pass bound
counts requests, so one slow creation cannot starve every later owner. No scope
freshness check runs inside a send (`LinearClient::graphql` sends what it is
given); freshness belongs to work boundaries (`tracker_events::sync_scope`,
`team_current`).

Request accounting lives in `logs/linear-requests.jsonl` when
`logs.linear_requests` is on (`lf history linear`).
