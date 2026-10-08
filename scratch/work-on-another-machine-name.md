# Share Task planning across machines — LOO-412

## Accepted direction — Jack Heart, 2026-10-08

Jack Heart selected local planning stores with bidirectional synchronization
through one custom Git ref, publication of PR #1491, and review in the existing
conversation. Steers `f883b34b-1195-4fc5-bfc7-1ac7d4c29be6` and
`1d12770f-96b1-4bc3-ba5b-1f863f4b9472` supersede host callbacks. The earlier
design is retained at `b040c7c8d:scratch/work-on-another-machine-name.md`; the
adoption demo remains at `0cd8e7f14` and in `remote-task-demo.md`.

LOO-412 owns Git transport and machine integration. LOO-406 owns the common
local SQLite planning writer and optional repository-wide Linear synchronization.
Only coherent committed APIs may be integrated, never its dirty implementation.
Ordinary operations do not call back to another machine or use another planner.
No real planning publication to the public code remote is selected. Prototype
acceptance uses synthetic records and disposable remotes. No landing or installation.

## Experience and preservation

```sh
lf --machine mini task create --title "Fix the parser"
lf --machine mini --task <task-selector> skill implement
```

Each machine reads and writes its own local plan. Synchronization shares stable
Task identity, briefs, comments and planning disposition in both directions.
Creation needs no checkout, Session or Flow. A later execution placement uses
the already pushed branch/commit; repeat launches reuse the worker's checkout.
Comments and completion propagate during ongoing work, with visible pending or
unconfirmed synchronization when disconnected.

Execution stays local. Never replicate Workflow position, Sessions, Processes,
claims, controls, checkout paths or PR execution state. Imported completion cannot
move a local Workflow, settle execution, signal a process or claim an exit.
Machine/account operations still act on the selected machine. No shared resident,
terminal relay, automatic turn/Flow retry, hidden arguments or cross-version
compatibility is included. No code, tests, help or config from herdr/cmux.

Ordinary creation establishes identity once; replication preserves it. Retrying
one creation retains identity while separate same-title creations remain distinct.
Existing divergent IDs and their execution histories are never renumbered. Use
provider mappings or the smallest explicit association where necessary; matching
titles do not establish identity.

Preserve fetch-before-decision and pushed-code checks. Report uncommitted source
work, missing branches and unpushed commits by branch/commit without committing,
pushing or resetting. Dirty/behind target checkouts retain files, HEAD, IDs,
Workflow, Sessions and PR history. A behind target reports `lf sync` in its
checkout. Creation without code has no fabricated commit or placement. The
one-shot code requirement constrains initial placement, not later local commits.

## Reconciliation and transport

Jack Heart's October 8 conflict decisions in LOO-406 supersede mandatory manual
winner selection: Linear wins observed planning conflicts. Without Linear, prefer
the host where appropriate, otherwise last-write-wins with try-not-to-clobber and
recoverable losing edits. Independent creations/comments accumulate and different
fields survive. A delayed completion must not overwrite an observed newer reopening.
The existing causal exchange module is an implementation starting point, not an
accepted requirement to expose manual conflicts or to block on LOO-406 APIs.
Simplify or replace it to implement the selected policy; retain original evidence.

Extend the common SQLite writers here with the peer ordering, stable identities and
atomic import/export checkpoint needed by Git synchronization. Export must not mint
new mutation identities. Import and projection commit together; fetched Git history
is not an import acknowledgement. Crashes before commit leave import retryable.
Pending state distinguishes local save, Git publication and optional Linear delivery.

`engine/planning_git.rs` transports a `planning.json` tree through
`refs/loopflow/planning`. Local retained/observed refs preserve unpublished and
incoming history. Fetch isolates each invocation's temporary ref without changing
source branch, index, checkout or `FETCH_HEAD`. Local saves compare the expected
tip. Publication never forces or blindly retries; readback distinguishes confirmed,
competing and unconfirmed outcomes. Concurrent history is reconciled before retry.
The 16 MiB document bound and 30-second Git deadline are prototype engineering
choices, not established production limits.

Integration still needs explicit planning-remote selection and semi-live invocation
synchronization. The public code remote is not implicitly an authorized planning
destination. The owning invocation and common writer supply synchronization;
no extra resident or execution driver. Local writes survive peer disconnection;
reconnect reconciles retained mutations and uncertain publication without replaying
turns. Losing a peer does not imply Task completion or provider exit.

## Delete — do not maintain

The superseded adoption walkthrough, screenshot set and capture scripts were
removed after verifying their bytes against `5960415b3`. The historical demo note
links that evidence; no replacement acceptance is implied.

- Replace retained `TaskSource.planning` in `ops/task/remote.rs` with common-writer
  peer import. The merge removed `accept_planning` and the cold Project bootstrap;
  delete the remaining copied `PmTaskRecord` payload with replacement consumers.
  Preserve branch/commit requirements and observation evidence.
- Replace `TaskId::from_issue` and its exclusive UUID-v5 dependency/tests with
  ordinary creation's portable identity. Preserve stored IDs and provider aliases.
- Move `task_source_args` and Task resolution to the integrated identity path;
  remove issue-only rewriting as a substitute for local-born identity.
- No planning callback implementation is present in this checkout. Do not restore
  the superseded full-store socket route. Replace adoption-specific fixtures while
  retaining their code-placement and preservation assertions on the surviving path.
- Update `docs/lf.md` and `docs/architecture/environment.md` when consumers move.
  They currently describe retained adoption, not completed custom-ref sync.

Changing dispatch before the replacement writer exists would leave an unusable
path. The deletion and replacement remain one coherent cut in this PR, without a
compatibility mode or another planner as an intermediate deliverable.

## Integration now selected — 2026-10-08

Jack Heart requested stacking on LOO-406 and continuing pursue. `lf task checkout
LOO-412 --stack-on LOO-406` selected PR #1503; `lf sync --manual` integrated its
published `e68f2a423bdabf91acadea72f23f01b3f383f244`. The common ownership cut
`84664e661` deletes `PlanningAuthority`, personal-plan storage and split writers.
Saved readers, ordinary creation/edit/comment/disposition and placement now share
local identity. This supersedes the missing-writer diagnosis at `4f9a8ea17`.

LOO-412 owns peer export/import, ordering and checkpoints on those writers. These
are implementation work here, not a new dependency for LOO-406 to deliver. Do not
wait for all Linear delivery to finish or create a second planner. Source evidence:
`e68f2a423:scratch/explore-loopflow-s-own-store.md`, common ownership, preserved
writers, accepted conflict policy and user-keyed planning sections.

The merge uses saved Task placement, retaining fetch-before-placement and source
branch/commit checks. Tasks may exist without checkout. The obsolete copied-record
importer and cold Project selector are removed; new peer sync must provide missing
planning before launch. Old transport payload/issue-derived helpers and adoption
fixtures still need replacement. This intermediate integration is not ready for
publication as a working remote path; complete the end-to-end cut before review.

Jack's accepted destination policy: user-keyed planning by default, explicit opt-in
to a common shared plan, same records/APIs. A common code remote never combines
plans implicitly. LOO-412 owns stable key provisioning/recovery and local remote/ref
binding; never derive identity from Git display name or a per-machine random user.
Joining an existing plan must not silently upload or merge the current local plan.
Shared ordering cannot grant an initiating host blanket precedence over other
collaborators. Choose deterministic ties and retain losers. Ref separation is not
access control; private planning needs an access-controlled destination. Synthetic
fixtures remain the only authorized publication target.

Remaining work is the replacement/integration cut, destination binding, semi-live
exchange with pending/error presentation, and public machine dispatch. Reuse 406's
local readers, field receipts and foreground sync patterns. Its known unseen-Linear-
write race is contrary evidence about provider atomicity, not a missing product
choice or reason to block independent peer integration.

## Acceptance for review

Use CLIs built from this worktree, isolated stores, synthetic plans and disposable
remotes. Installed-release behavior remains separate. Release's child memory
records a recovery path missed by lower-level fixtures; the same lesson requires
public command coverage here, not only exchange and transport tests.

1. Create/edit/comment/read from either machine through public commands. Replicate
   the same ID without placement from creation. Repeated deliveries create no
   duplicate Task/comment; separate same-title creations remain distinct.
2. Show semi-live comments/completion during remote work. Preserve populated
   Workflow, Session, Process and PR history/controls on imported completion;
   unrelated plans and progress remain usable.
3. Disconnect, write locally, show pending state, reconnect and converge without
   loss or turn replay. Lose a publication response and recover by readback. Crash
   after fetch but before import, then prove atomic retry.
4. Preserve conflicting edits/dispositions while independent updates propagate.
   Apply the selected deterministic policy and retain losing edits; delayed
   completion cannot undo observed reopening. Cover
   equal-value concurrent writes, conflicting ID reuse, omission and deletion.
5. Public `--machine` launches by issue/local selector and label/ID on a blank
   worker see pushed code and reuse placement. Cover local-born identity,
   divergent legacy IDs, unpushed source, missing branch and behind/dirty target.
6. Exercise local-only planning and contained optional Linear pending sync through
   the common writer. A local bare remote does not prove hosted custom-ref policy;
   that requires an explicitly selected disposable hosting repository.
7. Replace PR copy and create a walkthrough of final behavior and evidence;
   publish #1491 for Jack Heart's review and stop without landing.

Previous verification: 15 pure/Git transport fixtures plus fmt/Clippy passed before
stack integration; that proves neither SQLite import nor public dispatch. The
post-stack compile and focused proof are recorded below when complete. Gate owns
full integration acceptance after the replacement consumers exist.

Post-stack check (2026-10-08): `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`, and the worktree-built lib test `ops::task::tests::task_checkout_pins_upstream_without_requiring_clean_canonical_main` under `scripts/test_network.py` pass. Context fits. Legacy remote adoption fixtures compile but are not replacement-sync acceptance; pursue must replace them and prove the new path before publication.
