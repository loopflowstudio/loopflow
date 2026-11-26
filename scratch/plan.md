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
- **`before_first_read`** (`plan_carriers.rs:228`) owns the decision for
  both carriers (review F4) and runs them together, each to `WAIT`:
  - Git: when the repository has a remote and its Git carrier row does not
    name that remote with `has_read()`, `plan_git::exchange(.., Some(until))`.
    `plan_git::first_exchange` and its "Machine already knows the clone" test
    go; the decision moves here. `select_carrier` already resets `seen_seq`
    when the remote's name changes, so a clone pointed at another remote reads
    it on the next list.
  - Hub: when a hub is configured, `store.repository_id(repo)` is `Some`
    (after `lf config repo --bind`, `lf wt create`, a save) and the hub row
    does not satisfy `has_read()`, `plan_hub::exchange(.., Why::Read(until))`.
    Its `apply_plan_records(from: Some(..))` sets `seen_seq`, so it marks
    itself.
  - Cooldown on failure (review F3): when an unread carrier's row records an
    `error`, gate it on `begin_scope_check("plan-first-read:<repo>", now, 60)`;
    `Recent` skips it. Stamp the scope when a first read fails, so the second
    offline read is already skipped. A first read that works writes nothing
    to `planning_scope_checks`.
  - Every plan-reading CLI command calls it (review F2): `lf task list`
    (`lf.rs:999`), bare `lf repo tracker` (`tracker.rs:71`), and the
    direct-store paths of `lf plan list`, `lf plan board`, `lf wave list`,
    `lf wave status` and `lf wave plan` (`lf.rs:863`, `:915`, `:3253`–`:3297`;
    find the store-reading entry for each). On a read clone it costs two
    primary-key reads of `plan_carriers`.
  - Doc comments say "a clone that has not read its carriers' plan" instead
    of "a clone this Machine has not met".

No migration: the column exists and nothing else reads Git's `seen_seq`
(checked: `plan_hub.rs`, `test_fixture.rs:90` and `plan_carriers.rs:992` read
the hub's row only; no DTO or `lf repo tracker` output carries it).

### Choices made for review

Taken headless from `scratch/concept-review.html`'s recommendations; each is
reversible and none was answered by a person.

- **Reuse `seen_seq` rather than add a column** (F1). Same fact the hub keeps
  there; `select_carrier`'s reset on a changed peer comes free; LOO-1110's
  zero-idle-revision trigger holds. Git's `0` is a flag, not a watermark:
  the field doc and `rust/loopflow-storage/SCHEMA.md` say so. Passed over: a
  `first_read_at` column (migration plus a second spelling of the fact).
- **Every plan-reading CLI command does the first read** (F2, review 1a).
  Passed over: only `lf task list` and `lf repo tracker`, which leaves
  `lf wave list` first in a fresh clone empty for no reason a person can see.
- **A failed first read retries after 60 s** (F3, review 2a). `plan_git`'s
  backoff lives in process memory (`BACKOFFS`, `plan_git.rs:127`), so a CLI
  never sees it. Passed over: retry on every read (≤ 5 s stall on each
  offline list) and skip while `error` is set (an empty clone stays empty
  after reconnecting: LOO-1110's `exchange_unowned` only visits repositories
  with an open Task or Wave).
- **First read covers the hub too** (F4, review 3a). Passed over: Git only,
  which leaves the documented `lf config repo --bind` → `lf task list` empty.
- **A save still does not fetch.** The hub reads on a save (`Why::Save`), a
  fetch costs more, and the read path covers it. Passed over: making
  `plan_git::send` fetch when `!has_read()`.

### Effect on installed Machines

Every Git-carried repository's row has `seen_seq` NULL today. After upgrade
the first full exchange marks it; where no exchange has run since, the next
`lf task list` does one fetch (≤ 5 s) and marks it. Machines stuck in the
reported state heal on that list.

## Affected surfaces

- `lf task list`, `lf repo tracker`, `lf plan list`, `lf plan board`,
  `lf wave list`, `lf wave status`, `lf wave plan`: callers of
  `before_first_read`.
- Hub repositories: the first read asks the hub too.
- Linear/Asana repositories: unchanged, `carried` returns `None`.
- `docs/work.md:325` ("the first `lf task list` in a fresh clone reads the
  remote before it answers") becomes true; widen it to the plan-reading
  commands. `docs/dev/planning.md:284` says the first read "registers the
  clone for the server's minute", false since LOO-1110 for a clone with no
  open Task or Wave (review F5): rewrite that sentence to name the commands,
  both carriers and the 60 s cooldown.

## Absent and error states

- No remote: `first_exchange` returns `false`, nothing written.
- Remote with no plan refs: fetch succeeds, nothing to read, marked; the next
  list does not fetch.
- Fetch fails or is stopped at `until`: error recorded on the carrier row as
  now, not marked, scope stamped, `before_first_read` logs and the command
  goes on with the local plan. Reads in the next 60 s skip it; the first
  after retries.
- A ref that cannot be read: not marked; the others are taken in as now.

## Exclusions

- No change to backoff, the minute exchange's selection, or `send`.
- `exchange` registering the clone before a fetch that fails stays; it no
  longer hides anything.

## What goes

- The `store.repository_id(repo)?.is_some()` test in `first_exchange` and the
  "Machine already knows the clone" wording around it.
- `plan_hub.rs:93`'s direct `seen_seq.is_some()` (becomes `has_read()`).
- `plan_git::first_exchange`; its one caller was `before_first_read`.

## One pass

Nothing in `rust/` is built yet. One end-to-end pass: storage
(`has_read`, `plan_read`), `exchange_with` marks, `before_first_read` takes
both carriers and the cooldown, the five new callers, docs. It teaches
whether the cooldown and the hub branch fit in `before_first_read` without a
second resolver.

Tests, in `plan_git.rs`'s module beside
`a_task_made_on_one_clone_is_on_another_and_the_later_title_wins_on_both`
(move its `first_exchange` true-then-false assertion onto whatever replaces
it):

1. *A clone that saved before it read still reads on its first list*:
   `laptop.task("x")`, `laptop.exchange()`; `mini.task("made on mini")`,
   `mini.send()`; the first read runs, `mini.listed()` holds both titles, a
   second does not fetch. Fails on main: `listed()` is `["made on mini"]`.
2. *A remote with no plan is read once*: fresh `mini`, read then no read.
3. *An offline first read waits 60 s*: a remote path that does not exist;
   the first read fails and stamps; a second at `now + 1` is skipped; one at
   `now + 61` runs. Use a `now` parameter, not a sleep.
4. *A hub-only clone reads the hub on its first list*: in `plan_hub`'s test
   module, a bound clone with no Git remote lists the hub's Task after
   `before_first_read`.

And the CLI check, in `rust/loopflow/tests/work/local_planning.rs` with its
`lf(repo, home, args)` helper: one `TestRepo` bare origin, two homes. Home A
runs `lf task create --title A --json` in the `TestRepo` checkout. A second
`git clone` of `bare_path()` (repository-local `user.email` set, path
canonicalized) under home B runs `lf task create --title B --json`, then
`lf task list --json`, which names both A and B. A third home, `git clone`
then `lf wave list --json` first, names a Wave made on A.

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
- `lf wave list` / `lf plan list` first in a fresh clone shows the remote's
  plan; a hub-only clone lists the hub's Tasks right after binding.
- Offline, only the first read in a minute waits.
- Existing `plan_git` and `planning_sync` tests pass.

## Deferred findings

- **An empty clone never hears of a later first Task** (review F6). A clone
  whose first read saw an empty remote is marked read; LOO-1110's
  `exchange_unowned` skips repositories with no open Task or Wave, so a
  teammate's first Task arrives only on `lf task sync` or a lookup by ID.
  Review recommended 4a: keep empty clones quiet (LOO-1110's idle budget) and
  fix the docs that promise otherwise (folded into this pass's
  `docs/dev/planning.md` edit). Polling them would be its own Task.
- **`git remote set-url origin <new>` keeps `seen_seq`** (F7):
  `plan_carriers.peer` is the remote's name, not its URL. Would take tracking
  the resolved URL beside the name. Rare.

## Review notes

`scratch/concept-review.html` (from 91531a9eb2): F1–F4 adopted above as
choices made for review; F5's `send` doc fix (hub reads on a save) is done in
`plan_carriers.rs`, its first-read doc fixes are in this pass; F6, F7 deferred.
