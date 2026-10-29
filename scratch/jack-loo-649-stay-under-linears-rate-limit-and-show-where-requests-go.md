# LOO-649: Stay under Linear's rate limit, and show where requests go

Jack Heart requested this on 2026-10-27 after two Linear pauses in one day; the
Task definition is the accepted design. On 2026-10-28 Jack added the read-freshness
work and the complexity hypothesis (Task comments).

## Built (one choke point: `LinearClient::graphql`)

- **Request log** (`linear/request_log.rs`): off by default; on per Machine with
  `logs: {linear_requests: true}` in `~/.lf/config.yaml` (`config::machine_flag`,
  never a repository). One JSON line per request in `~/.lf/logs/linear-requests.jsonl`
  (rolls to `.1` past 32 MiB): operation, query/mutation, urgent, outcome
  (sent · rate_limited · paused · deferred · failed), HTTP status, duration, command
  label (`lf task status`, never argv), LfProcess, repository, target Linear id,
  `x-ratelimit-{requests,complexity}-{limit,remaining,reset}`, `x-complexity`.
- **Summary**: `lf history linear [--since 1h] [--json]` — outcomes, latest
  allowances, hourly totals, callers by command × operation with complexity.
- **Pause names the limit**: the cooldown warning in `lf.log` carries
  `limit=requests|complexity|endpoint-requests|unreported`, logging on or off.
- **Backoff** (`linear/cooldown.rs`): each response refreshes a shared budget file
  beside the cooldown deadline. Non-urgent requests are spaced at ≤25% of an
  allowance and held at ≤10%; held requests fail as `RateLimited` before sending.
  `linear::urgent(...)` covers creation export and PR links.
- **One read freshness check**: `linear_observe::team_current` runs the Team delta
  (no-op inside its window) and says whether local state answers. The window is
  10 s while the budget is healthy, one pacing interval when paced, until reset in
  the reserve (`delta_fresh_secs`). Routed through it: `lf task comment` refresh
  (Linear skips its per-issue trash + observe reads when current) and
  `inspect_task_planning_async` under `Auto`; `link_pr_async` now uses `Auto`
  instead of `Force`. Comments written in lf still push immediately.
- **PR link writes only on change**: an attach for a PR already linked as Open,
  with ids and no error, skips `attachmentUpdate` + `commentUpdate`.
- **TeamChanges complexity**: the delta selects `project { id }` instead of each
  issue's Project with `initiatives(first:50)` and `teams(first:50)`, pages 25
  instead of 100, and reads each changed Project once (`ProjectOwnership`).
  On paper an empty delta fell from ~20k points to a few hundred; the
  `x-complexity` column in the log confirms or refutes it.
- **Read paths audited**: `task status`, `task workflow` and history reads
  use stored Tasks and reach Linear only for a Task not yet stored
  (`acquire_task` / `inspect_task_planning_async` under `Auto`, so a current
  delta answers). Progress comments are unchanged until the log shows they lead.

## Remaining

- Turn logging on for Jack's MacBook, run a normal day, read
  `lf history linear --since 24h`; cut whatever still leads.
- Other audit candidates: up to 4 `sync_repository` per skill launch (absorbed by
  the window), progress comments (one `commentCreate` each).

## Delete — do not maintain

Nothing replaced: the cooldown stays the pause authority; the budget extends it.
`begin_linear_team_check`'s fixed 10 s became the budget-derived window.

## Checks

- Compress pass: `cargo test -p loopflow --lib -- linear::cooldown linear::request_log linear::team_changes linear_team_checks linear_on_demand native_runner_is_quiet attach_task_github_pr task_comment linear_observe`: 33 passed; clippy `--all-targets -D warnings` clean. Wider suites: gate.
