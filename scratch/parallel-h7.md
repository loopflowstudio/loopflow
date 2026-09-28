# H7 contribution — 2026-09-28

Status: implementing assigned files; no Cargo/build slot requested yet.
Jack's accepted status-based model in chapters.md governs. Existing dirty files
outside this allocation remain untouched.

## Approach and integration contract

Replace receipt-driven rotation with a repository preview and convergent provider
operation. Project status is the authority; one started Project selects the current
plan. New Projects use the explicit requested chapter name and a deterministic UUID
from the Wave's stable provider identity plus target name, so create/attach response
loss can be recovered by another Home without a Chapter receipt. Existing Planned
Projects are reused by their stable ID. No timestamp or newest-name selection.

Proposed ordering: create/attach successor, activate successor, transfer/cancel
predecessor Tasks, complete predecessor. Activation first avoids a zero-current
interval in this writer. Retry accepts requested target plus a single predecessor
name shared across the repository; unrelated competing predecessors remain errors.
A zero-current Wave cannot be silently assigned an arbitrary completed predecessor;
recovery requires an unambiguous shared predecessor established by the other Waves.
These are implementation assumptions, not additional decisions attributed to Jack.

Shared integration required (managed worker owns these edits):
- CLI `repo new-chapter NAME [--dry-run]` calls `chapter::new_chapter(repo, NAME,
  dry_run)` and renders the repository preview; remove Wave history/new-chapter and
  status --chapter plus Chapter DTOs/packet callers.
- PM/project planning carries required `flow: String` and ProjectStatus; migrate
  sync constructors and SQLite project columns. Do not restore recommended aliases.
- Remove Chapter store consumers in durable/children; Task admission selects current
  Project status. Preserve Started and retirement evidence during execution conversion.
- Migrations performance contributor must account for the new project_status_chapters
  draft in tests which currently assume wave_chapters survives at the draft head.
- Current Task start/claim retirement APIs remain, while Chapter receipt APIs go.

No configured provider writes, installed Home access, staging or commits. Final
note will name exact API deltas, patch, and pending focused verification commands.

## Shared sync handoff (source-ready; verification slot not granted)

`parallel-h7-shared.patch` contains one exact, unapplied change: call
`chapter::sync_projects(store, &registered, snapshot)` before publishing a PM
snapshot. This records Project flow/status and adopts same-Wave issue transfers
on a second Home without creating a Task or execution. The H7 fixture now asserts
this via ordinary planning refresh, rather than rerunning rotation on that Home.
The managed worker already integrated ProjectPlan constructors, SQL columns,
current-project admission, CLI routing and some DTO changes while H7 was editing;
do not replay earlier mechanical field patches over that work.

The draft is `project_status_chapters__9bea0388aa854e46a9db2d1e5c86ddfb.sql`,
depending on `one_flow_driver`. Its two columns are `projects.status` and
`projects.flow`. `work/chapter.rs` is deleted; classifier and ephemeral preview
shapes live in `ops/chapter.rs`. Store Chapter receipt APIs are removed. Task
Started/retirement/transfer functions retain their names for shared callers.

The public provider schema was checked against Linear's published SDK schema:
Project status categories include paused; `projectStatuses` has no filter, so
status discovery paginates and chooses the owning team's category before a
workspace category. Create uses explicit Planned status. `find_project` uses an
ID-filtered collection so not-found and a failed read are distinct. No configured
Linear endpoint or credentials were used.

## Ready for integration

Owned source is coherent and individually formatted. No Cargo command has run in
this contribution and no build slot has been granted. The shared PM source now
contains the `sync_projects` hook from the patch; this contributor did not apply
that shared edit. Retain the patch as the exact handoff, not a second application.

Implemented:
- Repository preview/application returns `ChapterRotation { name, waves }`; each
  `WaveRotation` names stable predecessor/successor IDs and Task dispositions.
- Planned successor content/identity is retained. New successors get explicit
  Planned status and a deterministic UUID; attachment and every Task/status
  mutation are confirmed by fresh provider evidence. A failed lookup is not
  treated as absence. Archived deterministic targets are explicit conflicts.
- Activation precedes transfer and predecessor completion. One target plus one
  common predecessor name is recoverable, including mixed Wave progress. The
  inverse flip's zero-current state is accepted only when another Wave establishes
  the same predecessor and the target is Planned. Arbitrary completed history,
  duplicate names and competing current plans remain unresolved. Final fresh
  reads require every Wave to select the requested name.
- Task transfer writes only same-Wave Project FK and update time. Backlog first
  retires under the local claim/Started/PR transaction, then is canceled in Linear;
  cancellation does not record native deletion. Local Done without provider
  settlement, unavailable checkout evidence, retired work with new start evidence,
  and active claims on terminal work stay unresolved.
- `ProjectPlan` and `PmProject` carry `flow` and `status`; `ProjectContent` carries
  `flow`. The old recommended wrapper and Chapter owner types/APIs are removed.
  Planning refresh records Project facts and same-Wave issue movement without
  starting Tasks. Documentation uses current repo/wave/task command names.

Implementation assumptions and retained integration work:
1. A Wave with no Projects starts with `flow: feature`; a sole Planned target may
   become its initial current plan. This is an implementation choice, not another
   decision attributed to Jack. Waves with historical Projects and no identifiable
   current predecessor do not guess a current plan.
2. **Initial provider adoption still needs completion before live cutover.** The
   draft preserves existing local Projects and initially marks them started, but
   normal provider sync replaces that status. A pre-H7 Project whose actual Linear
   status is Backlog/Planned is not automatically promoted to Started. Likewise,
   old `recommended:` content is not silently reinterpreted as `flow:`. Missing
   `flow:` stays an explicit empty value and prevents rotation before any provider
   mutation; new Project creation requires a nonempty flow. The accepted promise
   that existing non-archived Projects are current until first rotation therefore
   needs an explicit one-time provider-content/status adoption operation. Do not
   call this cutover complete by relying on the SQL defaults or replacing missing
   custom Flows with `feature`. No configured provider mutation occurred here.
3. The inherited classifier treats a provider backlog/unstarted issue with **no
   local Task** as untouched. The synthetic second Home proves convergence of
   that rule; it cannot prove another Home has no unreported checkout/Run. Cross-Home
   Started publication/retirement evidence remains an integration obligation before
   claiming that concurrent real Homes cannot retire each other's work.
4. Execution conversion still owns changing the retained `flow_invocations` claim
   queries to its final owner. Preserve the exact Task pointer, Started and PR
   fences; this contributor did not rename the parallel execution model.
5. Shared migration tests still read `wave_chapters` at the final head. They must
   retain old-prefix preservation checks but expect the table absent after this
   draft. Shared DTO/Swift fixture updates, exported chapter skills/packet deletion,
   complete existing PM fixtures and public CLI tests belong to integration.

## Verification and limits

Passed without building:
- `rustfmt --check --edition 2021 --config skip_children=true` over the eight
  edited extant Rust files (no global formatting).
- `git diff --check` over assigned paths.
- `uv run python scripts/check_migrations.py`: 54 shipped migrations unchanged,
  16 drafts; the new dependency/name/id are valid.
- `/tmp/loo298-h7-migration-proof.py`, run with `uv run --no-project python`:
  SQLite 3.50.4 replayed all canonical scripts and other source drafts, populated
  a Project with retained identity/content/timestamps/iteration and a Chapter
  receipt, then applied this draft. Project fields stayed identical, status/flow
  columns were added, the receipt table disappeared, and `foreign_key_check`
  was empty. This is SQL preservation evidence, **not** the Rust migration ledger,
  installed-Home compatibility, canonical materialization, or Task/Flow proof.
  The first harness attempt omitted the runner-created `schema_migrations` table
  and failed; the corrected harness initializes it and uses the runner's FK-off
  migration convention. No production code changed for that harness error.

Authored but **unexecuted** Rust evidence:
- Partial status flips, unrelated mixed-name/duplicate-target rejection, explicit
  zero-current ambiguity, empty/new Wave adoption, stable creation identity.
- Started/checkout/claim/retirement conflict classification.
- Synthetic GraphQL interruption after each of twelve mutations, retrying both
  same store and another private store; no duplicate Projects or lost issues;
  transferred issue IDs, canceled backlog and historical completed issues.
- A populated local Task keeps identity, checkout, Task plan, PR and captured Flow.
  After second-Home completion, ordinary PM refresh adopts its new Project.
- An authored Planned successor keeps its custom Flow and KR; unknown issue
  evidence refuses the operation before the first mutation.

First requested supervisor slot (use the existing bounded private-Home runner,
scrub inherited `LF_*`/`LOOPFLOW_*`, resource preflight, nice +10, four workers):

```sh
cargo nextest run -p loopflow --lib -j 4 --test-threads 4 --no-fail-fast -E 'test(ops::chapter::tests) | test(pm::tests::project_) | test(pm::tests::missing_flow_) | test(pm::linear::tests::list_projects_) | test(pm::linear::tests::create_project_) | test(pm::linear::tests::update_project_)'
```

After shared integration, run the affected migration suite and its populated
canonical materialization proof, affected PM/Task/DTO suites once, then
`cargo clippy --all-targets -j 4 -- -D warnings`. No configured Linear, installed
Home, actual second OS Home, desktop, native provider or public CLI proof is
claimed. The first command may reveal type/fixture integration errors; rustfmt
is a syntax check, not compilation. No staging, commit, rebase, publication,
installation, promotion or Flow navigation occurred.

Comparable assigned Rust counts before test modules: 4083 → 3916 lines
(net -167); excludes the separate test file and adds a 10-line SQL draft.
This includes nearby test-only helpers before those modules, excludes shared
integration, and does not count the moved classifier as deleted twice.
