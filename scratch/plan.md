# LOO-1113: the first `lf task list` reads the remote, even after a save

## Outcome

In a repository with no Linear or Asana, a fresh clone that runs
`lf task create` and then `lf task list` sees the other Machines' Tasks on that
first list. A clone that has fetched the plan refs once does not fetch again on
later reads.

## What is wrong today

`plan_git::first_exchange` (`rust/loopflow-execution/src/ops/plan_git.rs:339`)
decides "never read" by `store.repository_id(repo).is_some()`. Anything that
registers the clone makes that true without a fetch:

- `lf task create` → `ensure_repository`, then `plan_carriers::send` →
  `plan_git::send`, which pushes and never fetches (`plan_git.rs:369`).
- `exchange` itself calls `ensure_repository` *before* the fetch
  (`plan_git.rs:357`), so a first fetch that fails or is stopped at the 5 s
  `WAIT` also registers the clone and is never retried by a read.

Local Git state cannot stand in: after a send, `publish` writes this Machine's
own `refs/loopflow/plan-seen/<me>`, and a remote with no plan refs leaves
nothing behind after a real fetch. `plan_refs` is empty in that second case
too. The fact has to be recorded.

## Design

One fact per carrier and repository: **has this Machine read this peer**. The
hub already keeps it: `plan_carriers.seen_seq`, `None` before the first read,
and `plan_hub::exchange` tests `held.seen_seq.is_some()`
(`plan_hub.rs:93`). Git's row leaves the column NULL forever (only
`apply_plan_records(from: Some(..))` writes it, and Git passes `None`). Git
starts using it.

- **Storage** (`rust/loopflow-storage/src/sqlite/plan_carriers.rs`):
  - `PlanCarrier::has_read(&self) -> bool` (`seen_seq.is_some()`); the hub's
    check at `plan_hub.rs:93` calls it. The field doc says what it holds for
    each carrier: the hub's watermark; for Git, whose per-Machine places are
    `plan_refs`, 0 once a fetch was read.
  - `SqliteStore::plan_read(repo, carrier)`:
    `UPDATE plan_carriers SET seen_seq=0 WHERE repo=?1 AND carrier=?2 AND seen_seq IS NULL`.
    The `IS NULL` guard keeps an idle exchange from writing (LOO-1110: the
    `store_revision_plan_carriers_update` trigger compares every column, so
    the row's revision moves once, on the first read).
- **`exchange_with`** (`plan_git.rs:421`): after the fetch and the loop over
  the other Machines' refs, when no ref failed to read (`failed.is_none()`),
  call `store.plan_read(repo, Carrier::Git)`; before `publish`, so a refused
  push does not undo a read. Records not taken still count as read: they are
  on `not_taken` and the next exchange brings them. Every full exchange marks
  (`lf task sync`, the server's after-command and minute exchanges,
  `look_first`), so a repository the server has been reading is already marked
  before anyone lists.
- **`first_exchange`** (`plan_git.rs:339`): `false` when the repository has no
  remote, or its Git carrier row names that remote and `has_read()`. Otherwise
  `exchange(.., Some(until))`. One primary-key read of `plan_carriers`
  replaces one of `repository_plans`; no new cost on the list path.
  `select_carrier` already resets `seen_seq` when the remote's name changes,
  so a clone pointed at another remote reads it on the next list.
- `before_first_read` and its two callers (`rust/loopflow/src/bin/lf.rs:999`,
  `rust/loopflow/src/lf/commands/tracker.rs:71`) stay as they are. Doc
  comments on `first_exchange` and `before_first_read` change from "a clone
  this Machine has not met" to "a clone that has not read its remote's plan".

No migration: the column exists and nothing else reads Git's `seen_seq`
(checked: `plan_hub.rs`, `test_fixture.rs:90` and `plan_carriers.rs:992` read
the hub's row only; no DTO or `lf repo tracker` output carries it).

### Choices made for review

- **Reuse `seen_seq` rather than add a column.** It is the same fact the hub
  keeps there, and `select_carrier`'s reset on a changed peer comes for free.
  Passed over: a new `fetched_at`/boolean column (a migration plus a trigger
  clause for a second spelling of "has read this peer").
- **A failed first fetch is retried by the next read**, each bounded by
  `WAIT` (5 s). The brief asks that repositories that *completed* an initial
  fetch not repeat it; one that failed has not. Cost: a never-fetched clone
  with an unreachable remote pays up to 5 s on each `lf task list` and
  `lf repo tracker` until one fetch works. Passed over: skipping the read
  while the carrier row records an error, which is faster offline but leaves a
  fresh clone without the plan until `lf task sync` or a server exchange.
- **A save still does not fetch.** The hub reads on a first save
  (`Why::Save`), a fetch costs more, and the read path now covers it. Passed
  over: making `plan_git::send` fetch when `!has_read()`.

### Effect on installed Machines

Every Git-carried repository's row has `seen_seq` NULL today. After upgrade
the first full exchange marks it; where no exchange has run since, the next
`lf task list` does one fetch (≤ 5 s) and marks it. Machines stuck in the
reported state heal on that list.

## Affected surfaces

- `lf task list`, `lf repo tracker` (bare status): the two callers of
  `before_first_read`.
- Hub-only repositories: unchanged. `before_first_read` is Git's; the hub
  reads on the first save and after each served command.
- Linear/Asana repositories: unchanged, `carried` returns `None`.
- `docs/work.md:325` ("the first `lf task list` in a fresh clone reads the
  remote before it answers") becomes true in this case; no edit needed.
  `docs/dev/planning.md#without-a-tracker-stamps-and-carriers` gets a
  sentence if it describes when a first read happens.

## Absent and error states

- No remote: `first_exchange` returns `false`, nothing written.
- Remote with no plan refs: fetch succeeds, nothing to read, marked; the next
  list does not fetch.
- Fetch fails or is stopped at `until`: error recorded on the carrier row as
  now, not marked, `before_first_read` logs and the list goes on with the
  local plan.
- A ref that cannot be read: not marked; the others are taken in as now.

## Exclusions

- No first read for the hub in `before_first_read`.
- No change to backoff, the minute exchange's selection, or `send`.
- `exchange` registering the clone before a fetch that fails stays; it no
  longer hides anything.

## What goes

- The `store.repository_id(repo)?.is_some()` test in `first_exchange` and the
  "Machine already knows the clone" wording around it.
- `plan_hub.rs:93`'s direct `seen_seq.is_some()` (becomes `has_read()`).

## One pass

Tests, in `plan_git.rs`'s module beside
`a_task_made_on_one_clone_is_on_another_and_the_later_title_wins_on_both`
(its `first_exchange` true-then-false assertion keeps passing):

1. *A clone that saved before it read still reads on its first list*:
   `laptop.task("x")`, `laptop.exchange()`; `mini.task("made on mini")`,
   `mini.send()`; `first_exchange(mini)` is `true`, `mini.listed()` holds both
   titles, a second `first_exchange` is `false`. Fails on main: the first call
   returns `false` and `listed()` is `["made on mini"]`.
2. *A remote with no plan is read once*: fresh `mini`, `first_exchange` `true`
   then `false`.

And the CLI check, in `rust/loopflow/tests/work/local_planning.rs` with its
`lf(repo, home, args)` helper: one `TestRepo` bare origin, two homes. Home A
runs `lf task create --title A --json` in the `TestRepo` checkout. A second
`git clone` of `bare_path()` (repository-local `user.email` set, path
canonicalized) under home B runs `lf task create --title B --json`, then
`lf task list --json`, which names both A and B.

## Acceptance check

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow-execution --lib ops::plan_git:: < /dev/null
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow --lib local_planning:: < /dev/null   # the CLI check
cargo test -p loopflow-storage --lib plan_carriers::
cargo test -p loopflow-execution --lib ops::planning_sync::
```

## Demo

```bash
git init -q --bare /tmp/p/origin.git
git clone -q /tmp/p/origin.git /tmp/p/a && git clone -q /tmp/p/origin.git /tmp/p/c
(cd /tmp/p/a && LF_HOME=/tmp/p/home-a lf task create 'made on A')
(cd /tmp/p/c && LF_HOME=/tmp/p/home-c lf task create 'made on C' \
              && LF_HOME=/tmp/p/home-c lf task list)
```

The last list shows `made on A` and `made on C`. On main it shows only
`made on C`.

## Done when

- `lf task create` then `lf task list` in a fresh clone lists the other
  Machines' Tasks at once.
- A second `lf task list` runs no `git fetch`.
- Existing `plan_git` and `planning_sync` tests pass.
