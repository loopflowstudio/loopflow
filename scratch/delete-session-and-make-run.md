# PRD-38: Delete Session and make Run the sole executor

## Moment of transparency — July 18, 2026

The architectural cut is implemented on this branch, not yet landed.

Project and Task are stable Work identities. Their stored records contain
domain facts only. Epoch, Run, Launch, Turn, Wait, and Steer now provide the
entire execution lifecycle. The `task_sessions` and `project_sessions` tables,
their status enums, process generations, write leases, CRUD, recovery, and
Run-mirroring paths are deleted. Migration `0.11.036_delete_sessions.sql`
copies the surviving domain facts into `tasks` and `projects`, rewrites their
dependent records, then drops both tables.

One internal entrypoint starts every Project or Task executor:

```text
lf __work <project|task> <id>
        |
        v
run_work(WorkRef)
        |
        +-- resolve the exact ambient Run lease
        +-- prove lease.work == requested Work
        +-- dispatch Project or Task domain policy
```

The typed Project and Task loops remain because they conduct different domain
work. They no longer reconstruct stores, identity, lifecycle, or authority.
"One executor" means one durable execution spine and one authority-bearing
entrypoint, not one generic mega-loop that erases Project and Task policy.

## Intended behavior

```text
Work -> Epoch -> Run -> Launch -> optional Turn
                    \-> Wait

Steer advances Basis.
Run owns write authority.
Launch owns provider/process continuity.
Turn records an observable provider boundary.
```

- `WorkStatus` is derived as `Ready | Running(RunId) | Waiting(Wait) | Done |
  Abandoned`. There is no stored Task/Project lifecycle status to drift.
- a Run can end while Work remains ready or waiting; only the completion fence
  commits the Epoch `Done`;
- a successful boundary, current Basis, domain closure, and absent containment
  must all agree before completion;
- a new Steer or other revision racing completion makes the proposal stale;
- missing or stale in-Run credentials fail closed through `LF_RUN_CONTEXT` and
  the opaque Run lease;
- provider fallback creates another Launch, not another Work identity;
- Project and Task actions project one next legal action plus its reason. The
  old six-entry matrix of every blocked alternative is gone;
- roadmap and Swift DTOs expose `work_id`, `updated_at`, `work_status`, and
  `WorkStatus` directly. Clients no longer mirror Session status enums.

## Data normalization wins

The new shape makes these bugs unrepresentable:

| Old failure | Why it cannot exist now |
| --- | --- |
| Run active while Session says revoked | Session lifecycle state no longer exists |
| stale Session generation writes into a replacement body | the opaque lease identifies one exact active Run |
| Task/Project status disagrees with Epoch/Run/Wait | status is a projection of those records |
| process recovery releases an unproven writer | only positive containment absence ends the fenced Run slot |
| completion commits over newer direction | completion compares successful boundary Basis to current Epoch Basis |
| PM reopens work after premature completion | PM completion follows the same fenced Work completion path; reopen compensation is deleted |
| provider continuity changes Work identity | continuation belongs to Launch |
| Swift invents lifecycle from several flags | the wire sends one `WorkStatus` |
| surfaces disagree on which actions are blocked | the server emits only the one next legal action and reason |

## Deletion ledger

Deleted outright:

- Project/Task Session ids, records, status enums, write leases, process
  generations, and body records;
- `project_sessions` / `task_sessions` production storage APIs and SQL;
- Session-to-Run reserve, activate, settle, revoke, reap, retry, and recovery
  bridges;
- Project/Task lifecycle status writers and status event mirrors;
- Session-close / Epoch-quiescence synchronization;
- PM `ReopenTask` compensation and premature-completion repair paths;
- `__project` and `__task` executor commands;
- Swift `ProjectStatus`, `TaskStatus`, and Session-shaped roadmap fields;
- exhaustive blocked-action wire rows and their duplicated UI model;
- obsolete tests whose only subject was Session status transition plumbing or
  mock-call wiring.

Retained deliberately:

- `provider_session_id`, Claude `--resume`, tmux sessions, URLSession, and UI
  terminal surfaces: these name real provider/OS/client substrate concepts;
- historical migrations mentioning Session: old databases must be able to
  reach migration 36 before those tables are dropped;
- separate Project and Task policy loops;
- `child_control.rs` and `ops/child.rs`, because they now implement shared
  Work/Steer control rather than Session lifecycle;
- durable Run/Launch/Turn containment, race, and completion tests.

## Source result

The committed measurement is physical lines under `rust/loopflow/src`:

```text
baseline: 144,210
current:  134,190
removed:   10,020
```

The complete branch diff against `main` is currently +7,918/−22,596, net
−14,678 lines. The physical-source gate is intentionally stricter than net
diff: generated fixtures and cross-language migrations cannot hide additive
Rust architecture.

## Behavioral changes

- `lf task status` prints the one recommended action and reason instead of a
  list of every unavailable action;
- Work status JSON uses the durable tagged `WorkStatus` shape;
- Project/Task runtime DTOs rename Session-shaped fields to Work-shaped fields;
- completion no longer mutates a Task/Project status and later reconciles the
  Epoch; it commits only through `store.done(lease, basis)`;
- Run end does not imply Work completion;
- launch commands use `__work kind id` and reject a Run lease for other Work;
- migration 36 is one-way. There is no compatibility reader or dual write.

## Proof already run

- `cargo check -p loopflow --all-targets`
- `cargo test -p loopflow --no-run`
- fresh-database migration reaches the latest schema with neither Session table
- observed merged PR completes only across a successful Run/Launch/Turn fence
- all 11 durable Run/Launch/containment tests
- full Rust suite and `cargo clippy -p loopflow --all-targets -- -D warnings`
- full Swift package: 193 tests, including DTO fixtures, roadmap, attention,
  action, and lens behavior
- source measurement meets the 10,000-line floor

## Remaining dogfood

Exercise one configured Task through start, Steer, interrupt/recovery, and
completion when provider capacity permits. This is dogfood evidence, not a
substitute for the deterministic race proofs and not a publish blocker.

## Final architecture review

No blocking finding remains.

- The public model maps 1:1: Project/Task are product identity, Epoch is one
  attempt, Run is execution authority, Launch is containment/provider
  continuity, and Turn is an observable provider boundary.
- Product refresh writes cannot mutate Run-owned phase position. Reopen has an
  explicit full-state write because it creates a new Epoch.
- Run reservation owns the machine promotion fence. Interrupting the narrow
  reserved-before-Launch window ends the Run and returns no fabricated Launch.
- Migration history keeps its immutable Session-era names, rekeys historical
  Runs to stable Work ids, then drops both Session tables in one direction.
- The final diff contains no compatibility reader, dual write, lifecycle
  status mirror, or alternate child executor entrypoint.

## Done when

- [x] no production Task/Project Session table, type, status, CRUD, body
      generation, or recovery path remains;
- [x] Run is the sole executor authority and `__work` is the sole child
      executor entrypoint;
- [x] Project/Task domain state contains no lifecycle mirror;
- [x] completion is fenced by successful boundary Basis and containment;
- [x] provider continuity lives on Launch;
- [x] Work status and action surfaces derive from normalized durable evidence;
- [x] migration drops the old tables and fresh-schema proof checks the absence;
- [x] Rust source is at least 10,000 physical lines below the pinned baseline;
- [x] Swift and Rust compile against one wire model;
- [x] full Rust tests and clippy pass on final content;
- [x] final simulated architecture review records no blocking finding.
