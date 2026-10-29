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

## Concept review (2026-10-28, headless)

Actions → concepts → APIs match the accepted design: turn on a Machine flag →
request log → `lf history linear`; Linear's allowance → one budget per Home →
pace/hold non-urgent, `linear::urgent` scope for creations and PR links; reads →
one Team-delta freshness window → local state. No open product choice.

- Considered merging the cooldown deadline into the budget file (one record, one
  lock). Rejected: the released lf reads `locks/linear-cooldown/<key>` to honour a
  pause, so moving it would let an installed lf send through a pause.
- Remaining `LinearRefresh::Force` reads are deliberate: cold acquisition of a
  Task not yet stored (nothing local to serve) and chapter-rotation's
  `require_outside_current_chapter` (needs authority before moving work).
- `wave status` reaches Linear only through `sync_repository`, which the
  freshness window absorbs; Desktop monitoring (`monitor ps`) reads the store.

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
- Gate (2026-10-29): `scripts/test.py --reuse-passing` refused at resource preflight (12.1 GiB free < 32 GiB reserve; recovery retained other Tasks' builds). Ran instead: focused `cargo test -p loopflow --lib -- linear::cooldown linear::request_log linear::team_changes linear_team_checks linear_on_demand native_runner_is_quiet attach_task_github_pr task_comment linear_observe lf::commands::graph config::machine`: 39 passed (incl. new `finished_team_delta_answers_reads_inside_its_window`); `cargo fmt --check` and `cargo clippy --all-targets -D warnings` clean. `check_architecture.py` passes. Deferred to CI: `documented_commands::` (README names `lf history linear`) and the website suite.
