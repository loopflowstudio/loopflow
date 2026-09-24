# Task views — human direction, 2026-09-23

The human paused the demo to revisit essential information and speed, then asked
to relaunch with Active as the default. Active means the current working set: Tasks with an existing local worktree,
an open Session, or a live provider working in their checkout, including
interactive waiting. The human explicitly broadened this after reporting the
Linear OAuth Task flickering between activity polls.
Queries are an internal UI architecture, not text syntax exposed to the human.
Active and All tasks use one list renderer and one shared snapshot.

Open: future query operators, custom saved views/persistence and repository
breadth. No parser or user-authored query language is implemented. Do not infer
autonomous success from a Session or call a descriptive condition execution state.

Implemented: `TaskQuery` evaluates Active/All over the existing projection. The
window/repository navigation retains the selected query; switching queries reads
no CLI and changes no pane or Work selection. Shared `lf ps` now carries each
owned provider's recorded checkout from its Exec evidence. Rust still establishes
liveness and ownership; Swift compares that explicit checkout with the Task's
shared workspace reference. Unclaimed processes cannot activate a Task. Session
joins still use typed Work identity. This is workspace activity, not inferred
Run-to-Task attribution or execution success. Failed activity reads retain
last-good evidence and show an error; an initial read does not claim zero Tasks.

Verification: the focused final Swift query/DTO proof passes two tests; the Rust
activity boundary passes five tests, including checkout propagation, stale/dead
receipt exclusion and the shared DTO fixture. Receipts:
`/tmp/loo291-active-final-swift.log` and `/tmp/loo291-active-rust.log`. SwiftPM app
build passes. Broader gates remain separate. The demo uses a disposable signed
bundle and this checkout's newly built CLI; the installed app is untouched.

## Screenshot correction

The screenshot's `technical-architecture` warning comes from shared
`unavailable_projects`, not a failed planning read. Live evidence in
`/tmp/loo291-unavailable-roadmap.json` has one ready Project Work record absent
from the plan, with no attached Tasks. Absence alone does not authorize deleting
or abandoning that record.

Moved this diagnostic from the primary navigator into Wave details, including
any recorded Tasks that remain outside the plan. Current planning and explicit
read errors/partial warnings stay in the list. No provider/PM mutation or extra
read was added. Two focused view tests pass: diagnostic relocation and existing
failed-read retention, `/tmp/loo291-planning-detail-proof.log`. The already-open
demo binary predates this edit; no live user terminal was restarted to replace it.

## Worktree retention correction

The 21:42 human screenshot shows LOO-291, LOO-293, LOO-285, LOO-279 and
LOO-226. The user reports LOO-279 appearing/disappearing. Its recorded local
worktree exists even though planning says Completed. Active now includes that
existence independently of provider liveness, so provider gaps change the badge
without removing its row. Completed planning work can remain in the working set;
it is still labeled Completed. An old Run alone does not establish membership.

The existing shared Task workspace reference carries optional `local_exists`
from a filesystem check on the reading Home; failure is unknown, never false.
Swift performs no filesystem I/O in rendering and introduces no poll or store.
Unknown checks produce an incomplete-Active warning and suppress healthy emptiness.
The same shared reference serves status and roadmap. Existing additional local
worktrees may expand the list beyond the five initially observed rows.

The original empty demo had two distinct problems: versioned/path-with-spaces
lf binaries were rejected despite exact live process receipts, and the demo
was pointed at the development registry rather than the live Home. The receipt
check now uses exact PID/start evidence and the disposable demo selects
`/Users/jack/.lf`. Neither correction infers provider ownership from a title.

Focused proof: `worktreeRetainsActiveTask`, `workDoesNotRequireSessions` and
the Swift roadmap DTO fixture pass (3 tests, exit 0),
`/tmp/loo291-worktree-retention-swift.log`. Rust's shared status/roadmap fixture
also passes (1 test, exit 0), `/tmp/loo291-worktree-dto.log`.
The local-worktree row badge is a subsequent presentation-only change; the app
build includes it. Review retained exact Session joins and one workspace owner;
there is no sticky row cache, timeout, execution-success inference or extra reader.
