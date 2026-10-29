# LOO-649 demo: Linear request log, summary and backoff (2026-10-29, headless)

No one was present; this pass stood in for Jack Heart at the demo step. Design:
`scratch/jack-loo-649-stay-under-linears-rate-limit-and-show-where-requests-go.md`.

## What ran

Branch build `target/debug/lf` (rebuilt from 777eff7be; the earlier debug binary
predated `lf history linear`).

- **Real home (`~/.lf`)**: `logs.linear_requests: true` is already set there. The
  branch binary routes a real-home command to the home's installed release
  (0.15.0), which has no `history linear` ("unrecognized subcommand") and writes no
  request log. So the real-Machine part of the demo — a normal day of autonomous
  work read back with `lf history linear --since 24h` — can only run once this
  lands and is installed. That is the Task's "Done when" proof and belongs to verify.
- **Temp home, logging off**: `lf history linear` prints the exact config switch
  to turn it on and "No Linear requests recorded"; `logs/` holds only `lf.log`.
- **Temp home, logging on, synthetic 57-line log** (sent, deferred, urgent, mutation
  rows): text summary shows outcomes, latest requests/complexity allowances with
  reset, hourly sent/held/complexity, and callers by command × operation ranked by
  spend; `--json` carries the same fields.
- **Stand-in endpoint tests** (recording, off default, backoff, delta freshness,
  PR-link skip, trimmed TeamChanges): see Checks.

## Interpretation (stand-in judgment, not Jack's words)

The CLI surface reads as designed: one screen tells you which command and
operation spent the budget, and held requests are visible separately from spent
ones. No design change surfaced. The one gap — no live data yet — cannot close
before release because real-home commands run the installed lf.

## Next

Land, install, run a normal day on Jack's MacBook (switch already on), then
`lf history linear --since 24h`: confirm hourly totals stay under the limit and
read `x-complexity` on `TeamChanges` to confirm or refute the trim.

## Checks

- `cargo test -p loopflow --lib -- linear::cooldown linear::request_log linear::team_changes linear_team_checks linear_on_demand attach_task_github_pr task_comment linear_observe`: the demo rerun was stopped while still compiling, before any result; the compress pass recorded 33 passed on this code. Gate owns the rerun.
