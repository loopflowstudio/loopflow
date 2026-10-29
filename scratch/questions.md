# LOO-649 assumptions (headless run, 2026-10-28)

- Machine-level switch is `logs.linear_requests` in `~/.lf/config.yaml`, not an
  `lf set` key: `lf set` values are repository/Wave scoped and would cost a store
  read per request.
- "Task" attribution is the Linear id the request names (`issueId`/`id`), not a
  resolved lf Task id.
- Reserve 10%, pacing from 25%. Urgent = creation export and PR links; everything
  else is non-urgent by default.
- With a current Team delta, `lf task comment` refresh skips the per-issue trash
  check, so a coworker's deletion is noticed when the delta is stale instead.
  Issues owned by another Team than the repository's are not covered by its delta.
- PR link skip covers only Open → Open with no prior error. Merges and abandonments
  are recorded outside the linking path, so Merged/Abandoned always write.
- `delta_fresh_secs` reads the production endpoint's budget; test stand-ins see a
  healthy (10 s) window.
- TeamChanges trim was made on the page-size math without live `x-complexity`
  data (no logging was on yet); the log can confirm it.
