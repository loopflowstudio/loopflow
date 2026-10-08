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

`engine/planning_exchange.rs` retains current and superseded change identities
per portable field. Observed reopening supersedes completion; concurrent values
remain explicit conflicts. Equal concurrent values retain both causes. A resolution
is a new write observing all alternatives, never a clock or Git merge-base choice.
Wave/Project membership travels as a pair. Comment IDs suppress duplicate delivery;
omission never deletes. Explicit deletion retains identity, content and evidence.

The common writer must persist causal context atomically with each ordinary
mutation. Export reuses those IDs. Import commits causal state, projections and
its checkpoint together, preserving conflicts while independent updates proceed.
Fetched Git history is not an import acknowledgement. Crashes before SQLite commit
leave import retryable. Pending effects distinguish local persistence, Git
publication confirmation and optional Linear synchronization.

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

- Replace `TaskSource.planning`, `TaskSource::accept_planning` in
  `ops/task/remote.rs`, and the `ops/task.rs` bootstrap call with common-writer
  peer import. Delete the copied `PmTaskRecord` owner in the same integration cut;
  preserve branch/commit requirements and observation evidence.
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

## Missing dependency and remaining implementation

LOO-406's committed head checked on October 8 is
`4f9a8ea17020dfe3fe21c1d199e60d6602a57059`, newer than the previously inspected
`a3324396b`. Placement and Flow entry now share `require_task_launch` /
`require_task_planning`, consuming saved planning in SQLite without provider
resolution. Missing inventory permits saved work; retained invalidation, removal
and membership mismatch still apply. Terminal planning prevents new work while
an active Workflow retains its position. Integration must reuse this admission
path rather than restore provider lookup or copied-planning bootstrap.

`planning.rs` still declares Local/Linear authority and
`store/sqlite/local_planning.rs` branches on it. Its migration retains creation,
comment and state-delivery records, but no causal peer import/export/checkpoint
contract. This admission change is not the common-writer cut; no dependency merge
has occurred. The dependency's committed memory also records a still-failing
`task_completion_preserves_linear_reopening_during_delivery`: unconditional
Linear delivery can overwrite a concurrent reopening and matching readback can
falsely acknowledge it. LOO-406 owns that repair/investigation. Git ancestry
confirmation establishes publication only; it supplies no SQLite import or
optional Linear conflict-preservation proof.

Required coherent committed API from LOO-406:

1. Ordinary create/edit/comment/disposition writes save stable mutation IDs,
   causal context and pending synchronization in the same SQLite transaction.
2. Export reads those identities without reminting them; peer import atomically
   merges facts, retains conflicts and records the import checkpoint.
3. Planning disposition can be imported without changing local Workflow or
   execution. Existing IDs, aliases, comments and history survive import.

After that boundary exists, LOO-412 still needs the deletion/integration cut,
remote selection, semi-live synchronization, pending/error presentation and public
machine dispatch. No dependency merge or ordinary sync activation has occurred.
Fifteen pure/Git fixtures cover reconciliation and transport, not SQLite import
or public dispatch. The independent transport work cannot substitute for this
missing common writer.

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
   Resolve explicitly; delayed completion cannot undo observed reopening. Cover
   equal-value concurrent writes, conflicting ID reuse, omission and deletion.
5. Public `--machine` launches by issue/local selector and label/ID on a blank
   worker see pushed code and reuse placement. Cover local-born identity,
   divergent legacy IDs, unpushed source, missing branch and behind/dirty target.
6. Exercise local-only planning and contained optional Linear pending sync through
   the common writer. A local bare remote does not prove hosted custom-ref policy;
   that requires an explicitly selected disposable hosting repository.
7. Replace PR copy and create a walkthrough of final behavior and evidence;
   publish #1491 for Jack Heart's review and stop without landing.

Reconciled 2026-10-08: causal merge skips retired changes before copying values;
its regression retains conflicting reuse of a change ID through replay and explicit
resolution. Per-change value sets are necessary. Only tests consume exchange and
transport; public dispatch still consumes adoption. The newer LOO-406 admission
path changes the integration target, not the missing-writer boundary. Existing
Rust checks below remain applicable because this reconciliation changes prose only.

Check (2026-10-08): retained `cargo test -p loopflow --test planning_exchange_tests --test planning_git_tests --no-run` build, both binaries via `uv run --no-sync python scripts/test_network.py` (15 passes), fmt and Clippy passes; prose reconciliation: `git diff --check` and `lf context --skill realign --json` pass within budget. Integration acceptance remains with gate after the common writer is available.
