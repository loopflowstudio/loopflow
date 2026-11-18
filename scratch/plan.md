# LOO-938 — One plan at every size

The design is `scratch/plan-design.md` (guide: `scratch/plan-design.html`).
This file is how the branch gets there, and where the two differ this file
holds. History of the question: `scratch/plan-ownership-history.md`; what was
said: `scratch/explore.md`; the review that set the three differences below:
`scratch/concept-review.html`.

*Made for review, differing from the design:*

1. **A Machine's ref holds its whole plan**, every field with the stamp it
   carries, whoever wrote it. The design says a ref holds only the fields whose
   newest stamp is that Machine's and prunes the rest. That breaks when a
   teammate reaches a hub but cannot push: everyone prunes on seeing their
   stamp and Git never carries the value. Whole-plan refs need no pruning, no
   merging of fragments across refs, no held-back tip, and a retired Machine's
   ref can simply be deleted. Cost: each change is re-published by every
   Machine as one small commit (identical records are one blob on the remote).
2. **One shape for both carriers.** A carrier sends whole stamped records and
   keeps one cursor into `plan_changes`. `hub_sends` goes; both lines say
   "unsent" and count records.
3. **The ref names the repository**, so a fresh clone needs no
   `lf config repo --bind` (checked in slice 2).

"Your order of your claimed Tasks" (the design's second order) moves to the
tracker Task; see Exclusions.

Jack Heart, 2026-11-18, on whether the design replaces "the hub owns the
plan": "Why woudd it not? it seems like it does". On order: "your order of
your claimed tasks is NOT the proejct order, ut your order of hte backlog IS".

## What people see

- One Machine: nothing changes. `lf repo tracker` reads `Plan: this Machine.`
- Two Machines sharing a remote: a Task made on the laptop is on the mini
  within a minute, with no setting. `lf repo tracker` on the laptop reads
  `Plan: Git origin, 1 unsent.` then `Plan: Git origin.`
- With `lf machine hub mini`: the same change arrives in seconds, and a second
  line reads `Hub mini.` or `Hub mini, unreachable since …, 2 unsent.` Stop the
  mini's server and Git still carries it.
- A teammate who clones the repository sees the plan and their Tasks appear
  on yours. No account, no hub.
- Two edits to one field: the one made later wins on every Machine. Nothing
  is ever `lost` or `conflicted` between Loopflow Machines.
- Reordering the backlog on one Machine reorders it on all of them
  (`task_rank` is a stamped field like any other).
- Linear and Asana repositories behave as they do on main.

## Source of truth

Each Machine's store, plus one stamp per field. The plan a Machine shows is,
for each field, the value with the newest stamp it has received. Three
carriers move stamped values and none of them decides anything.

### The stamp

`(at, machine id)`, compared in that order. `at` is
`max(wall clock in ms, highest at this Machine has written or seen + 1)`.

*Made for review.* The design says "one more than the highest this Machine has
written or seen". With that alone, a laptop offline since counter 4000
retitles a Task on Tuesday at 4001 and loses to the mini's Monday retitle at
4500, because the mini did more writes. Folding the wall clock in keeps the
design's guarantee (an edit made after seeing yours beats yours, whatever
either clock says) and makes unseen edits resolve by when they were made. A
Machine with a clock an hour fast wins unseen races for that hour and no
longer: everyone who sees its stamp moves past it.

A field with no stamp row reads as `(0, this Machine)`. So nothing is seeded:
a store that predates stamps offers its whole plan at 0, and any save beats
it. *Passed over:* stamping existing values from the Git journal's clocks (the
design's "Moving what exists"). It reads tables this branch marks leaving, and
after the cutover below the Machines agree on every value anyway.

### Tables (all in this Task's one migration, `plan_hub.sql`, still unmerged)

- `plan_stamps(owner_id, field, at, machine_id)`, primary key
  `(owner_id, field)`, index on `at` for the clock. New.
- `plan_changes` stays: the local order in which records changed. A local save
  bumps its record; so does an arriving value that wins. An arrival that
  changes nothing bumps nothing, which is what ends an exchange.
- `plan_carriers(repo, carrier, peer, sent_seq, seen_seq, error,
  failing_since)`, primary key `(repo, carrier)`, `carrier` is `git` or `hub`.
  `sent_seq`: how far into this Machine's `plan_changes` the carrier has
  taken. `seen_seq`: the hub's `plan_changes` watermark (NULL for Git).
  `peer`: the remote name or the hub's machine id. It replaces `hub_plans`,
  `hub_sends` and their triggers; it keeps the `store_revision_*` triggers so
  views see the lines change.
- `plan_refs(repo, machine_id, tip)`: the last commit of each other Machine's
  ref taken in. Per store, not per clone, so two clones of one repository on a
  Machine agree.

"Unsent" for a carrier is the count of records in `plan_changes` past its
`sent_seq`. No per-field queue and no `queued` / `sent` phase: sending a
record twice changes nothing.

### One record shape

A stamped record is a record's id, its table and every field it has, each with
`{value, at, machine}`. `plan.apply`, `plan.changes` and a file in a Git ref
all carry exactly this, from one serializer (`planning_write::read` plus the
stamps). One function lists the stamped records past a cursor; both carriers
call it.

### One way in

`planning_write` gains the only function that takes values from elsewhere:
stamped records in, each field written (as `WriteOrigin::Import`) only when
its stamp is newer than the one held, stamps recorded, clock moved past the
highest seen. The hub's import, `plan.apply` on the hub and the Git import all
call it. A local save stamps every field it changes with the next stamp; a
tracker observation is stamped the same way.

It replaces `import_hub_plan`'s skip of fields with an open send, first
contact's "queue every owner the hub lacked", and `apply_hub_edits` writing
arrivals as local saves. `comments_arrived` (ownership markers, direction
steers, re-claim) stays and runs for every carrier.

## Carriers

**Hub.** `ops/plan_hub.rs`, protocol revision 9, never released, so changed in
place. One exchange: send the stamped records past `sent_seq` with
`plan.apply` (paged by the existing byte budget) and move `sent_seq`; read
`plan.changes` since `seen_seq` and apply. Order does not matter. The first
exchange sends the whole plan once. `lf machine hub --clear` deletes the hub
rows and loses nothing.

**Git.** `refs/loopflow/plan/<machine id>` on the repository's remote: this
Machine's whole plan, one stamped record per file (`tasks/<2 hex>/<id>.json`,
`waves/<id>.json`, `projects/<id>.json`, `comments/<2 hex>/<id>.json`),
canonical JSON so equal records are equal blobs, plus `repository` at the root
holding the repository id. One exchange, in any checkout of the repository:

1. `git fetch <remote> '+refs/loopflow/plan/*:refs/loopflow/plan-seen/*'`.
2. For each other Machine's ref whose tip moved: `git diff-tree` from the tip
   in `plan_refs` (the first time, or when that commit is not in this clone,
   from this Machine's own last tree, else the empty tree), read the changed
   files, apply through the one way in, record the tip.
3. Build this Machine's commit with `git fast-import` on top of
   `plan-seen/<me>`: rewrite the file of every record in `plan_changes` past
   `sent_seq`. Push; on success move `sent_seq`. Offline, nothing is committed
   and the line reads `N unsent`.

Only the owning Machine pushes its ref, so a push is a fast-forward. A ref of
a Machine that is gone can be deleted by anyone once any Machine has fetched
it; this Task adds no command for it.

Probe (git 2.50.1, `/tmp`): a first ref of 10,000 records built in about 1 s
including generating the input; one changed record and one deletion on top
took 0.12 s with no index and no worktree; the pushed ref does not appear in
`git status`, `git branch -a` or a plain clone; a wildcard fetch into
`plan-seen/*` and `diff-tree` between tips listed exactly the two paths.

A repository with no remote writes no ref. *Passed over:* the design's "the
ref exists locally" for one Machine; nobody could read it.

**Repository id.** *Made for review.* When a fetched ref's `repository` differs
from this clone's, the clone takes the lower of the two through
`bind_repository`, so every clone converges without a command and the hub
finds its checkout. Checked in slice 2: binding keeps rows saved under the
clone's first id, so this stands.

**When they run.** The Machine server owns both. After a command it answers,
the existing coalesced exchange (`after_command`, quiet 5 s) does the hub and
the Git push. One server task runs the Git exchange for every repository
without Linear or Asana that has a remote, every 60 s. `lf task sync` does
both in the foreground. A command run with no server sends to the hub as now
and leaves Git unsent until the server or `lf task sync`. *Made for review:*
60 s, one `git fetch` per repository per minute, no setting.

**Linear / Asana.** Unchanged in this Task: `tracker_actions`, per Machine.

## Surfaces that change

- `lf repo tracker` text and `--json`: `plan: {git: {remote, unsent,
  error, failing_since} | null, hub: {machine, unsent, error,
  failing_since} | null}`, with the unsent records listed under each line by
  identifier. The per-field `queued` lines go. DTO fixture
  `tests/fixtures/dto/tracker_sync.json`, Rust and Swift fixture tests.
- Desktop `RepositoryPlanView` / `RepositoryPlan.swift`: the same two lines.
- `machine/wire.rs` `PlanApply` / `PlanChanges`, `server_schema.json`,
  Swift `ServerProtocol`.
- `lf task sync`: prints what is unsent by each carrier.
- Docs, written ahead of the build (write-docs, 2026-11-18) and describing
  the behavior above: `docs/work.md` ("Share the plan", the Sync reference),
  `docs/ops.md`, `docs/config.md`, `docs/glossary.md`, `docs/dev/planning.md`
  (Words; "Without a tracker: stamps and carriers"), `docs/dev/machines.md`,
  `docs/dev/reference.md` ("Shared plan" row), `docs/dev/data.md`,
  `rust/loopflow/src/server/README.md`. Implement reads them as the spec for
  wording: `Plan: Git origin, 1 unsent.`, `Hub mini.`, records listed by
  identifier, Git's own reason on the error line. Code links in the reference
  row (`HubPlan`) follow the types.

  Both things the docs assumed held: `lf task claim` / `lf task run` exchange
  first and the losing claim's run stops, keeping its work; a clone with a
  remote needs no `lf config repo --bind`.

  Questions writing exposed:
  - A teammate who can fetch but not push sees the plan and every save of
    theirs stays unsent, with a push error on the line for good. Should a
    clone that has never pushed successfully read differently (say
    `Plan: Git origin, read only.`) instead of an error?
  - `docs/work.md` now says "anyone who can push to it can change any Task".
    Does `docs/security.md` need the same sentence?

## Absent and error states

- No remote, no hub: `Plan: this Machine.` Nothing runs.
- Fetch or push fails: `Plan: Git origin, unreachable since …, N unsent.` with
  Git's reason; the hub line is independent. Saves never wait on either.
- Push rejected because the remote's own-Machine ref moved (a second store
  using this Machine's id): shown as the error; never forced.
- A file that does not parse, or a field this build does not know: that field
  is skipped and logged; the rest of the ref applies.
- A hub older than this build: the existing skew error naming the fix.
- A released `lf` on another Machine keeps reading the old
  `refs/loopflow/planning/*` and sees no new changes. `TrackerConfig::ReleasedGit`
  stays until LOO-973.

## Operational boundary

10,000 Tasks, about 15 fields each: `plan_stamps` near 150,000 rows, read by
primary key; the clock is `max(at)` on its index. Check both and the
`plan_changes` range read with `EXPLAIN QUERY PLAN` in `query_plans.rs`.
A change costs one small commit per Machine on the remote and one no-op diff
read per Machine; sized for about ten Machines, not a hundred.
Stamping happens inside the save's own transaction: no second write lock. Git
runs outside any SQLite transaction. One exchange per carrier and repository
at a time (`locks/plan-hub/<id>.lock`, `locks/plan-git/<id>.lock`); a hub and
a Git exchange may overlap, and each import is one transaction.

## Acceptance check at gate

Extend `machine_commands::the_later_change_shows_on_both_machines_through_the_hub`
(stand-in `ssh`, real carrier) to three homes and a bare remote:

```bash
L: lf task create 'x'            # remote unreachable → repo tracker: "Git origin, 1 unsent"
L: lf task sync ; M: lf task sync ; M: lf task show <id>     # title "x", no hub named
L: lf machine hub mini ; M: lf task edit <id> --title one
L: lf task sync ; L: lf task show <id>                       # "one"
stop M's server ; L: lf task edit <id> --title two           # Hub mini, unreachable, 1 unsent
T (third home, another identity): lf task sync ; lf task show <id>   # "two", by Git
start M's server ; M: lf task show <id>                      # "two"
```

and the case the hub-owns design needed "pull before push" for: L holds a
stale title and has never met the hub; after first contact both show M's
newer one. And the case whole-plan refs exist for: T's push is refused, T
retitles through the hub, a fourth Git-only home sees T's title from L's ref.

## Cutover for etude

By hand, not code. With the released `lf`: `lf task sync` on the laptop and
the mini until `lf repo tracker` shows both refs confirmed (the laptop's
personal ref showed "local changes pending" and "Git acquisition failed" on
2026-11-18; the 22 records there exist only on the laptop and will arrive on
the mini as new records either way). Install on both. Each server writes its
ref within a minute. Neither Git nor the hub needs `lf config repo --bind` on the mini, once the
mini has fetched. Unverified: the mini was unreachable during both kickoffs, so its
etude checkout and `lf` version are unknown.

## Exclusions

- A person's slice of a team's Linear Project, Wave membership from the link,
  Linear manual order for the backlog, a person's order of their claimed Tasks
  (a rank keyed by person and Task, shown only to them), and a person's own
  Machines exchanging through Git on a tracker repository. This needs its own
  Task, LOO-980: it needs "which refs are mine" and a decision on which
  rule owns a field both a tracker and a stamp carry. The two orders are
  designed together there. Two things said for it on 2026-11-18 are in
  `scratch/explore.md` and the design's tracker section: backlog reorders
  are written in each tracker's own terms, and a Team or Initiative matters
  only as the default for a new Wave.
- A command that deletes a gone Machine's ref, and squashing a ref's history.
- Who may use someone's hub beyond SSH reach.
- The phone, presence, the hub's persistent connection: LOO-861.
- Task identifiers for repositories without a tracker.

## What goes

- `import_hub_plan`'s first-contact branch and open-send skip;
  `apply_hub_edits`; `hub_sends`, `HubSend`, `open_by_owner`,
  `mark_hub_sent`, `confirm_hub_sends`, the `queued` / `sent` phase;
  `hub_plans` (into `plan_carriers`); `forget_hub_plans` reporting values
  "the hub never took".
- "The change sent last wins" for the hub: gone from code and docs. The
  phrase stays where it describes Linear/Asana (`planning/rules.rs`,
  `docs/dev/reference.md` "Linear events", `docs/dev/delivery.md`), which
  this Task leaves alone.
- Already deleted on this branch and staying deleted: the journal, field
  causality, destinations, `PlanningSync`, `lf repo tracker key` / `journal`,
  the `two-people` World. The Git carrier is new code in a new module; nothing
  comes back from `planning_git.rs`.
- Must survive: `lf machine hub`, the hub line, `comments_arrived`, the cold
  move sending the plan to its target first (`session_move::ready`), every
  Linear/Asana path.

## Slices

1. **Stamps and one record shape, over the hub.** *Built.* `plan_stamps`,
   the stamp on every local and tracker save, `StampedRecord`, the one way in
   (`planning_write::apply`), `plan_carriers` with the hub's cursor,
   `plan.apply` / `plan.changes` carrying records, `hub_sends`, `hub_plans`
   and first contact deleted, `lf repo tracker`'s hub line in records, Swift
   model and view. The storage module is `sqlite/plan_carriers.rs`.

   What it taught:
   - The model holds: later edit wins in either arrival order, a save after
     seeing a fast clock beats it, a stale laptop takes the hub's newer value
     at first contact, and a re-sent record changes no row (storage tests with
     the clock set, `sqlite::plan_carriers::`).
   - `plan_changes` is bumped when a *stamp* is adopted, not only a value.
     Two Machines that wrote the same value keep different stamps otherwise,
     and a third edit between them splits the plan.
   - What a peer just gave is not owed back: an import from a carrier moves
     that carrier's `sent_seq` to the end when nothing was unsent before it.
     Without this `lf repo tracker` reads `N unsent` after every read. That
     holds for the hub only. The Git import passes no carrier
     (`apply_plan_records(.., None, ..)`): a whole-plan ref owes every arrival
     back in this Machine's own ref, and the same exchange publishes it. A Git
     arrival also counts as unsent for the hub, which is how one carrier feeds
     the other.
   - A page of records carries what each record needs ahead of it (a
     comment's Task, a Task's Project, a Project's Wave, a Wave's parent and
     selected Project), so any page applies alone. The old send split parents
     from children at a page boundary and would have stalled. A plan ref's
     files are read through the same import, in one call per fetched ref, so
     Git gets this from `create`'s parents-first order.
   - Unstamped fields converge on the higher machine id at first contact, and
     the lower one adopts every stamp once (its whole plan counts as changed,
     then is marked sent to that carrier).
   - `plan_changes` had no `store_revision_*` triggers and
     `store_revisions_cover_every_table` was failing on the branch; it has
     them now, since the unsent count is read from it.
   - `--json` is `plan: {hub: {machine, unsent, records, error,
     failing_since} | null}`: `unsent` is a count and `records` names the
     first twenty. `git` joins it in slice 2.
   - Without Git, the first line is `Plan: this Machine.` and the hub line
     follows it. `lf machine hub` alone prints `Hub mini.` or `No hub.`
   - Not built: skipping a field this build does not know. A hub older or
     newer is caught by the protocol revision; a plan ref file needs it
     (slice 2), which means parsing fields one at a time, not a typed
     `Vec<StampedValue>`.
2. **Git refs.** *Built* (`ops/plan_git.rs`, `plan_refs`), except etude's
   move, which is by hand. The same records written as files, fetch, import,
   the server's timer (`server/maintenance.rs`), the repository id check, the
   Git line in `lf repo tracker`, `lf task sync` and Desktop, the acceptance
   test with a bare remote and a third home.

   What it taught:
   - The repository id check passes: `bind_repository` keeps the first id as
     a local locator and rows saved under it stay with the checkout, so a
     clone takes the lowest id any fetched ref names. The `repository` file
     stays and the docs need no `--bind` for a repository with a remote.
   - A clone that has saved nothing must still read: the exchange registers
     the repository itself (`ensure_repository`). Skipping an unregistered
     clone left a second Machine with an empty plan.
   - A Machine whose ref is missing from the remote publishes its whole plan
     whatever its cursor says, so a deleted ref or a new remote heals.
   - The exchange commits only when the tree would differ, so two Machines
     that agree stop writing commits (checked: refs unchanged after extra
     exchanges).
   - Between convergence on a lower repository id and the hub's own fetch,
     the hub is addressed by an id it has not seen and answers "no checkout"
     for up to a minute. The acceptance test runs mini's fetch first.
   - A second clone of one remote on the same Machine cannot take the shared
     id (`bind_repository` refuses an id that locates another path); it keeps
     its own and logs. Untested.
   - The last case of "Acceptance check at gate", the one whole-plan refs
     exist for, is in the acceptance test: a home whose push is refused
     retitles through the hub; a fourth Git-only home reads that title from
     the laptop's ref.
   - A record skipped because it could not be written (questions, slice 2)
     is not retried: the ref's tip is recorded once read, and later diffs
     start from it. It arrives again only when its file next changes.
   - Reads that are not asked for do not fetch: `lf repo tracker --subscribe`
     and a provider exit exchange with the hub only; `lf task sync`,
     `lf wave status --sync`, a Task lookup that misses, a served command and
     the server's minute do Git too.
3. **Claims are first-wins.** *Built*, without Linear or Asana
   (`sqlite/task_ownership.rs`, `ops/plan_git.rs` `before_claim`). An entry
   carries `order` and `seen`; every Machine sorts the log by `order`, then
   the writing Machine's id; a claim whose `seen` is not the entry in effect
   just before it loses to the claim holding the Task and names no owner
   anywhere. The Machine whose claim lost writes a steer naming who was first
   and a Task interrupt. `lf task claim` and placing an unowned Task exchange
   by each carrier first, five seconds at most.

   What it taught:
   - The order cannot be read off the comment's stamp. A move hands its claim
     to the target before the comment arrives, and with no carrier in reach
     the comment may never arrive, so the entry carries its own `order`: the
     plan clock, and past every entry its writer held for the Task.
     `journal_order_in` is gone; a marker that arrives with a move keeps the
     order its writer gave it.
   - `seen` names the last entry *in effect*, not the last in the log. Naming
     a claim that lost would make the next entry look stale on a Machine that
     has not received the loser yet.
   - Only a claim loses, and only to a holder. An unassign that crossed a
     takeover still unassigns, and the taken-over Machine claims again, as on
     main.
   - Seeing first does not stop a second run by a teammate: after the
     exchange `lf task run` takes the Task over as on main, and the earlier
     owner's Sessions are told. On your own Machines it does: the claim is
     refused with "ownership is written on Machine mini".
   - Routing (`work_route::resolve`) reads known Work only, so the exchange
     sits at the claim, after the command has chosen this Machine.
   - Not built: showing a lost claim as lost in `lf task status`; `lf ship`
     and checkout adoption exchanging first.

## Checks

- Slice 1, run by implement, passed 2026-11-18:
  `cargo nextest run -p loopflow-storage --lib -E 'test(sqlite::plan_carriers::) | test(sqlite::planning_write::) | test(query_plans::) | test(migration_runner::) | test(revisions::)'`;
  `cargo nextest run -p loopflow -p loopflow-execution --lib --test integration -E 'test(the_later_change_shows_on_both_machines_through_the_hub) | test(store::children_tests::) | test(store::ownership_lifecycle_tests::) | test(planning_setup_tests::) | test(dto_fixtures::subscribe_frames) | test(server::schema::tests::)'`;
  `cargo clippy -p loopflow-storage -p loopflow-execution -p loopflow --all-targets -- -D warnings`.
- Those ran before main (#1974, the `processes` table goes) was merged in at
  `be214dee63`. After the merge only
  `cargo check -p loopflow-storage -p loopflow-execution -p loopflow --all-targets`
  ran, clean, 2026-11-18. Slice 2 reruns the hub acceptance test as it
  extends it; the storage tests rerun at gate.
- Slice 2, run by implement, passed 2026-11-18:
  `cargo test -p loopflow-execution --lib ops::plan_git::`.
  `cargo nextest run -p loopflow --test integration -E 'test(the_later_change_shows_on_both_machines_through_the_hub) | test(dto_fixtures::subscribe_frames)'`
  (the acceptance test: 137 s);
  `cargo nextest run -p loopflow -p loopflow-storage --lib -E 'test(server::schema::tests::) | test(revisions::) | test(migration_runner::)'`
  (one migration test finished at 299 s of nextest's 300 on a loaded
  Machine). `server_schema.json` regenerated.
- Slice 2's tests ran before main (#1977, Session reads) was merged in at
  `5796dd348b`; the merge touched no plan file. After it,
  `cargo check -p loopflow-storage -p loopflow-execution -p loopflow --all-targets`
  ran clean, 2026-11-18. The acceptance test reruns as slice 3 extends it.
- Slice 3, run by implement, passed 2026-11-18:
  `cargo nextest run -p loopflow -p loopflow-execution -p loopflow-storage --lib --test integration -E 'test(server::schema::tests::) | test(task_ownership::tests::) | test(store::ownership_lifecycle_tests::) | test(store::ownership::tests::) | test(task_ownership_follows_claim_handoff_and_unassign_through_the_writer) | test(of_two_claims_made_apart_the_first_holds_on_both_machines) | test(dto_fixtures::) | test(sqlite::plan_carriers::) | test(revisions::)'`
  (101 tests; `store::ownership_lifecycle_tests::a_double_claim_…` failed as
  the old rule's test and is rewritten as `of_two_claims_made_apart_…`, which
  passes); the acceptance test
  `test(the_later_change_shows_on_both_machines_through_the_hub)` with the
  refused-push case, 62 s; clippy on the four crates. `server_schema.json`
  and `SCHEMA.md` regenerated.
- Seen once, cause unknown, for gate to watch: before those passes,
  `machine_commands::of_two_claims_made_apart_the_first_holds_on_both_machines`
  and the acceptance test timed out at nextest's 300 s twice (load average
  about 90, together and then the claims test alone), and the claims test
  then passed in 28 s and 19 s with nothing changed. The same commands by
  hand took 1 to 7 s each. `ps` showed stopped (`T`) `lf` processes of other
  sessions on the Machine at the time. No hang was found in the claim path.
- Slice 3's run by hand, 2026-11-18 (two homes, a bare remote in `/tmp`):
  both `lf task claim`, three `lf task sync`; `lf task status` on both showed
  `Owner: ada`, both claims in the log, and the losing home held the steer
  "ada on home_… claimed this Task first, before this Machine had seen it…".
- For gate, not run here: the rest of `machine_commands::`, `transport::`,
  `binary(transport_tests)`, `documented_commands::` (docs changed),
  `performance_tests::`, `local_planning::` and the Task launch fixtures
  (placing an unowned Task now exchanges first, which pushes a plan ref to a
  fixture's remote), `migration_runner::` (the ownership trigger is recreated
  with `seen`), and Swift
  `scripts/test_desktop.sh --filter 'DTOFixtureTests|DesktopHeadlessTests|WorkModelTests'`
  (`RepositoryPlan.swift`, `RepositoryPlanView.swift` and three test files
  changed with the `--json` shape, `GitCarrier` and `CarrierStanding`, and
  were never compiled on this branch).
- Slice 3's tests ran before main (#1980, Codex usage limit) was merged in at
  `3f4fde8d14`; the merge touched no plan or ownership file. After it,
  `cargo check -p loopflow-storage -p loopflow-execution -p loopflow --all-targets`
  ran clean, 2026-11-18. No test was rerun.
- Left on the Machine, not started by this pass as far as it can tell: five
  `target/debug/lf server run` processes of this checkout, one to five hours
  old.
- Next: nothing in this Task's slices; gate runs the list above. Open choices
  are in `scratch/questions.md`.
