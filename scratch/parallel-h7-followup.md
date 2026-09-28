# H7 follow-up — ready for shared integration

2026-09-28 · LOO-298 · Bounded Codex contribution requested by Jack Heart.
Owned source and tests are edited; shared integration is an **unapplied patch**.
No Cargo/Swift build, worker, installed Home access, provider request, staging,
commit, rebase, publication or installation ran. No Rust behavioral pass is claimed.

## Four repairs

1. `successor_id` retains the Initiative/name hash but sets RFC 4122 variant and
   version **4** bits. The provider fixture rejects supplied non-v4 IDs, and the
   deterministic-identity test checks both fields. Existing response-loss tests
   still exercise reuse of the same ID. No configured v8 Project was created by
   this contribution or the original fixture contribution.
2. `disposition` starts missing local Task evidence at `authored=None`, not
   `Some(false)`. Backlog remains unresolved. The new two-Home test gives one
   Home a real dirty checkout while the issue remains Unstarted; the other Home
   has no Task row, refuses rotation and leaves provider status/membership intact.
   The old twelve-mutation recovery matrix now explicitly seeds clean local Task
   checkouts on both fixture Homes before expecting cancellation. Its former
   missing-row cancellation expectation was invalid and is removed.
3. Linear decoding projects archived records as Completed (retaining Canceled),
   even when the raw provider status remains Started. Membership requests include
   `archivedAt`; point lookup already requested it. IDs, content and issue history
   remain. The archived-predecessor fixture omits the archive from membership,
   returns it through retained-ID lookup, selects the actual current Project and
   checks the old Task/PR/capture remain intact without provider mutation.
4. The H7 draft retains pre-upgrade identity evidence **before** dropping
   `wave_chapters`. Existing Project rows receive nullable `legacy_current`:
   `1` recorded current, `0` recorded noncurrent, `-1` no receipt. New rows have
   NULL. Previously observed planning-only Projects are materialized from cached
   snapshots; receipt-only IDs are retained too. Existing rows keep their IDs,
   timestamps, progress and ownership. Cached snapshots convert `flows.recommended`
   to required `flow`/`status`, so an offline cached read still decodes.

The temporary marker owns no Chapter, content or execution. A marked Project's
fresh content is converted once, preserving its prose, KRs and custom Flow.
Ordinary parsing has no `recommended:` alias. Planning reads project the pending
conversion without provider writes. Explicit sync or rotation writes converted
content and the required current status together through `AdoptProject`, confirms
the fresh provider result, then clears that Project's marker. Lost replies leave
the marker retryable. Other Homes with the same retained old receipt converge on
the already-converted provider facts. Deliberately Planned successors stay
Planned. Archived history is never promoted.

A recorded current identity selects a Backlog/Planned predecessor. Without a
receipt, only one eligible retained Project with no competing Backlog/Planned
Project can be adopted as current. A fresh Started Project prevents reactivation
of an old predecessor. Ambiguity remains explicit; names and dates do not choose.
Missing/conflicting Flows stay errors, never `feature` substitutions. These are
implementation decisions within the upgrade promise, not new approval attributed
to Jack.

## Shared integration — apply once

[parallel-h7-followup.patch](parallel-h7-followup.patch) contains only:

- `ops/pm.rs`: overlay exact pending conversions before canonicalizing validated
  Project reads; run provider conversion during explicit non-plan sync **before**
  its existing rename/content writer can discard an old custom default.
- `store/migrations.rs`: populated preservation test using the existing
  `apply_before_current_draft`/`current_draft_sql` helpers, including materialized
  migration support. It checks stable identity/progress, a custom Flow, a Planned
  successor, cached DTO decoding, deleted Chapter table and foreign keys.

The original shared H7 sync patch is already integrated. Do not replay it.
Without this new read overlay, the newly authored upgrade tests cannot pass and
first-upgrade planning still sees provider Backlog/Planned and empty Flow. This is
a concrete remaining integration step, not a completed runtime claim.

Owned files changed: `ops/chapter.rs`, `ops/chapter_tests.rs`, `pm/linear.rs`,
`store/chapters.rs`, `store/sqlite/chapters.rs`, the existing
`project_status_chapters` draft and `docs/waves.md`. All other production paths
remain with the managed worker. Source syntax checks of patched shared files ran
on temporary copies only.

## Checks actually run

| Check | Result and boundary |
| --- | --- |
| `rustfmt --edition 2021 --config skip_children=true --check` on the five edited Rust files | Pass; syntax/format only, no type checking |
| `rustfmt` on temporary patched PM/migration files | Pass; originals untouched |
| `git apply --check scratch/parallel-h7-followup.patch` | Pass against current shared source |
| Scoped `git diff --check` | Pass |
| `uv run --no-project python scripts/check_migrations.py` | 54 shipped migrations unchanged since v0.12.23; 17 drafts valid |
| `uv run --no-project python /tmp/loo298-h7-followup-migration-proof.py` | Pass on SQLite 3.50.4, replaying canonical SQL and source drafts with the exact new populated test seed; Project facts/custom Flows/current and future markers/cache conversion preserved, Chapter table absent, FK check empty |
| `uv run --no-project --with graphql-core python /tmp/loo298-h7-followup-schema-validation.py` | All eight operations validate, including `AdoptProject` and membership `archivedAt`, against the supervisor's retained official schema |

SQL replay does not exercise the Rust ledger, compiled decoder or release
materialization. GraphQL validation does not establish runtime authorization,
UUID acceptance or provider convergence. Provider tests below are written, not
executed. The schema receipt is
`/tmp/loo298-h7-followup-schema-validation.json`; it records source/schema hashes.

## Managed-worker build slot

Apply the shared patch, then use the existing bounded runner and resource
preflight, disposable Homes, scrubbed LF/LOOPFLOW authority, nice +10 and four
workers. Do not run these concurrently with another Cargo batch.

```sh
cargo nextest run -p loopflow -j 4 --lib --test-threads 4 --no-fail-fast -E 'test(ops::chapter) | test(adoption_converts_only_the_legacy_flow_key_and_preserves_authored_content) | test(project_status_adoption_preserves_current_identity_and_custom_flow)'
cargo fmt --all --check
cargo clippy --all-targets -j 4 -- -D warnings
```

The Chapter filter includes the existing twelve-provider-mutation interruption
matrix and these new cases:

- `a_second_home_cannot_expire_backlog_with_unobserved_work`
- `archived_predecessor_is_history_even_when_linear_still_says_started`
- `legacy_project_adoption_preserves_plans_across_lost_responses` (Backlog and
  Planned origins, each of three conversion writes interrupted, same/second Home,
  idempotence, then ordinary rotation preserving Task checkout/PR/capture)
- `legacy_adoption_without_a_receipt_never_guesses_between_plans`

Repeat the populated migration test in the supervisor's release-materialized
tree. The shared test deliberately resolves the draft marker rather than
including an ordinal-free draft path. Existing H7 fixtures were adapted to the
correct missingness contract; no failure was hidden by retaining false negative
work evidence.

## Review findings and exact limits

- No local Task, inaccessible checkout, unknown workflow state or missing
  provider read authorizes expiry. Positive Started/claim/authored/PR evidence
  still transfers unfinished work. Terminal provider evidence remains historical;
  a live local claim conflicts rather than being silently discarded.
- For a *present* local Task, expiry still relies on a clean checkout at the
  recorded base, no local Started/claim/publication evidence and eligible provider
  state. The SQLite retirement write fences local execution/PR facts, not remote
  Homes or concurrent filesystem edits. Wave placement is not a global database
  lock. This follow-up does not establish absence of remote work from a copied
  local Task row; the two-Home cancellation matrix supplies explicit local facts
  on both Homes and must not be cited as proving that stronger claim.
- Provider status/content writes are not cross-Home transactions. Fresh reads
  and confirmation retain retry/conflict evidence; a simultaneous unrelated
  external editor can still race a provider write. No new shared owner or
  distributed lock was introduced.
- Adoption requires retained identity evidence (Project rows, cached snapshot or
  old receipt). A Project never observed by the upgrading Home has no marker and
  its legacy content is not silently treated as a current plan. Multiple plausible
  old plans remain unresolved. Missing receipt-only provider IDs likewise remain
  errors, preserving the evidence instead of inventing a replacement Project.
- `legacy_current` is cleared per Project after confirmation. It remains an inert
  nullable schema column afterward; no Chapter table, plan packet or ongoing
  dual-format reader was added. A future cleanup can drop the transition only
  after supported upgrade frontiers no longer need it.
- The retired helper's UUIDv8 shape was never a supported live-provider identity
  contract here. UUIDv4 bits preserve deterministic retry within this new writer;
  no compatibility search over both generated IDs was added.

Code review found and repaired a shared sync hazard: rename used the ordinary
parser before migration and could erase `recommended: custom`. The patch moves
conversion ahead of that writer. No configured provider/desktop acceptance,
whole-branch compile result or full LOO-298 completion follows from this handoff.
