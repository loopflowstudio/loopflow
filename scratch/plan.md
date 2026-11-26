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
    `lf wave show`, `lf wave status` and `lf wave plan` (`lf.rs:863`, `:915`,
    `:3253`–`:3367`; `lf wave show` shares `waves::show` with `lf plan list`).
    On a read clone it costs two primary-key reads of `plan_carriers`.
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

- **`--subscribe` reads the store alone** (post-build F3, review 2a), all
  five commands, including `lf task list --subscribe`, which waited for the
  first read on main. A fresh clone opened only by subscription stays empty
  until a one-shot read or `lf task sync`. Passed over: a background first
  read after frame 1 (2b), blocking like main (2c).
- **Any carrier read means the clone is read** (post-build F1, review 1a);
  built in pass 2.

### Effect on installed Machines

Every Git-carried repository's row has `seen_seq` NULL today. After upgrade
the first full exchange marks it; where no exchange has run since, the next
`lf task list` does one fetch (≤ 5 s) and marks it. Machines stuck in the
reported state heal on that list.

## Affected surfaces

- `lf task list`, `lf repo tracker`, `lf plan list`, `lf plan board`,
  `lf wave list`, `lf wave show`, `lf wave status`, `lf wave plan`.
- Hub repositories: the first read asks the hub too.
- Linear/Asana repositories: unchanged, `carried` returns `None`.
- Docs: `docs/work.md` ("Through Git", "Seconds, not a minute: a hub") and
  `docs/dev/planning.md` ("When they run", `plan_carriers` `seen_seq` note).
- `--subscribe` (all five) reads the store alone; see choices below.
- `lf task detail <id>`, `lf task show <id>`: sync on miss, like
  `lf task status <id>`.

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

## Built

- ba211424c3: storage `has_read`/`plan_read`; `exchange_with` marks Git read
  before `publish`; `before_first_read` with the 60 s cooldown; every
  `--subscribe` store-only; docs.
- b9ab216a14 (pass 2): clone-level gate in `first_read_with` (either current
  carrier read → no carrier runs); `before_first_read` called from the
  non-subscribing entry points in `waves.rs`/`tracker.rs`, skipped for served
  requests, none left in `lf.rs`; `lf task detail <id>` syncs on miss.
- Realign after the third review (421e210d0b), small fixes:
  - `carried_store` (`plan_carriers.rs`) opens the store on a Machine's first
    command when the clone has a Git remote, for `before_first_read` and
    `exchange_after_command`. Before, `lf wave show` (no `admit_process`) on a
    brand-new Machine printed an empty plan; the other reads only worked
    because admission had created `loopflow.db` first.
  - The cooldown applies only when every eligible carrier's row has an error
    (`first_read_with`), so an offline hub's leftover error no longer holds
    back a Git remote that came back.
  - `screens::registered_task` is the one sync-on-miss lookup;
    `lf task detail` and `lf task show` (`desktop::read_task_pane`) share it.

## Tests (built)

In `plan_git.rs`'s module beside
`a_task_made_on_one_clone_is_on_another_and_the_later_title_wins_on_both`:

1. A clone that saved before it read still reads on its first list.
2. A remote with no plan is read once.
3. An offline first read waits 60 s (`now` parameter, no sleep).
4. A hub-only clone reads the hub on its first list (`plan_hub` module).

5. Git + hub fresh clone: read #2 runs neither (`plan_hub` module).
6. Bound clone, hub failing: Git marks, read #2 runs neither.
7. Offline hub beside a Git remote that came back: Git runs inside the
   minute (`an_offline_hub_does_not_hold_back_a_git_remote_that_came_back`).

CLI checks in `rust/loopflow/tests/work/local_planning.rs`: B's
create-then-list names A and B; a brand-new home's first `lf wave show` shows
Wave `alpha` with A's and B's Tasks; fresh homes' `lf task detail <id>` and
`lf task show <id>` find A's Task. All pass locally.

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
- On a Machine with a hub, a fresh Git clone's second read touches no
  carrier; an offline hub never stalls a clone Git has read.
- `lf task detail <id>` and `lf task show <id>` on a fresh clone find a
  remote Task; `lf wave show` first on a brand-new Machine shows the plan.
- Existing `plan_git` and `planning_sync` tests pass.

## Deferred findings

- **An empty clone never hears of a later first Task** (review F6). A clone
  whose first read saw an empty remote is marked read; LOO-1110's
  `exchange_unowned` skips repositories with no open Task or Wave, so a
  teammate's first Task arrives only on `lf task sync` or a lookup by ID.
  Review recommended 4a: keep empty clones quiet (LOO-1110's idle budget) and
  fix the docs that promise otherwise (folded into this pass's
  `docs/dev/planning.md` edit). Polling them would be its own Task.
- **`lf task detail` answers local-only fields on the first sync-on-miss
  call** (third review, F2 aside): `Read::TaskDetail` checks
  `Read::peer_route()` before the miss is resolved, so a Task owned by another
  Machine gets a forward only from the second call. Would take running miss
  resolution before `peer_route`, or rechecking it after. Not a first-read
  failure: the Task is found.
- **`git remote set-url origin <new>` keeps `seen_seq`** (F7):
  `plan_carriers.peer` is the remote's name, not its URL. Would take tracking
  the resolved URL beside the name. Rare.

## Review notes

First review (91531a9eb2): F1–F4 adopted as choices above, F5's doc fixes
done, F6–F7 deferred. Post-build review (a00847808a): F1 (clone-level gate)
and F2 (one owner, `lf task detail`) built in pass 2; F3 (`--subscribe`
store-only) kept. Third review (421e210d0b, at b9ab216a14): F1 (`lf wave show`
empty on a brand-new Machine), F2 (`lf task show` sync on miss) and F3
(cooldown `||`) fixed in this realign (see Built); its F2 peer-route aside is
deferred above; F4 restates the `--subscribe` choice; F5–F6 are the empty
clone and `set-url` deferrals. Its questions 1–4 match choices already made
for review (1a now built).
