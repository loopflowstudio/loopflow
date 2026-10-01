# LOO-336: re-evaluate usage attribution after a bind

Status: implemented 2026-10-01. Jack Heart requested the evaluation on
2026-09-30; the attribution rule itself is unchanged and remains his choice.

## Outcome

Jack can see, from real Session history, how often Sessions are bound after
work began and how much Task and Wave usage would move under post-hoc
attribution, beside a written recommendation.

## Approach

- `lf usage --binds [--json]`: read-only comparison in
  `lf/commands/bind_attribution.rs`. Steps of a bound Session are `before`
  (finished before the bind under another owner), `during` (running or
  unfinished at the bind) or `after` (recorded to the Task). Task and Wave rows
  show the recorded total beside the total with `before` and `during` added.
- `SqliteStore::bound_sessions` lists Sessions with a bind time.
- `performance/bind-attribution.md` holds the reading, affected reports and
  recommendation. Wave memory and the architecture reference point to it.
- No change to the history reader, stored events or schema.

## Delete — do not maintain

Nothing. The comparison adds a reader; no predecessor exists.

## Remaining

- Real reading: none possible on 2026-10-01. Installed 0.12.28 has no
  `lf session bind` and no Session tables. Rerun `lf usage --binds` after a
  release with Session history has been in use, then Jack decides.
- If Jack picks post-hoc: change the owner selection in
  `conversation_inputs` (`store/sqlite/sessions.rs`) and delete this report.

## Checks

`cargo test -p loopflow --lib bind_attribution` and `bind_preserves_prior_work`: pass; `cargo clippy --all-targets -- -D warnings`: pass. Affected suites: gate.
