# H7 repair — unapplied patch ready

2026-09-28 · LOO-298 · Two findings from `scratch/reviews/parallel-h7-review.md` only.

[parallel-h7-repair.patch](parallel-h7-repair.patch) changes four files on
application. No implementation file was edited here; all construction and
formatting used private copies. No build, behavioral test, provider call,
installed-Home access, Git mutation, delegation or Flow action ran.

## Changes

- Explicit full PM sync calls a new narrow `LinearClient::rename_project`.
  Its GraphQL input contains only `name`, and it checks provider success.
  Legacy adoption still converts `recommended:` before normalization; rename
  never serializes description, prose, KRs or Flow. Other authored-content
  updates retain their existing operation.
- Rotation consumes the already-fetched final inventory before completing the
  predecessor. The selected successor must still have its stable identity,
  requested chapter name and Started status; the predecessor must be Started
  or Completed. Missing records and observed conflicting statuses refuse the
  completion write. Already-Completed predecessors skip that write and retain
  the existing confirmation/read-back path. The existing third-current-Project
  check remains. No new provider read, lock, table or owner is introduced.

## Regression fixtures included, not executed

All three use the existing Chapter provider/store fixture:

1. `explicit_sync_renames_legacy_projects_without_rewriting_authored_content`
   calls public `pm_sync` twice with `plan: false`. A migration-marked Planned
   Project has bare name `previous`, custom description, prose before/after the
   old Flow section, `recommended: custom`, and a KR. The final provider object
   must equal its original except for canonical name, the adopted Started status
   and the exact `recommended:` → `flow:` replacement. Cached custom Flow/KR,
   Task Project identity, PR and captured Flow are checked too. The public sync
   runtime runs on a blocking thread under the existing test-local PM context;
   no production visibility or test-only entry point was added. The provider
   fixture also models the old full UpdateProject writer, so the regression can
   expose lost content rather than merely rejecting an unexpected mock call.
2. `rotation_preserves_conflicting_statuses_observed_in_the_final_inventory`
   injects old Canceled/Planned or new Canceled/Planned/Completed immediately
   before the first inventory after Wave A's dispositions finish. It asserts the
   conflict, preserved provider statuses, retained Task transfer/cancellation,
   and no completion of Wave B's predecessor. The injection is gated by actual
   fixture state, not a brittle count of reads. Existing fixtures opt out.
3. `rotation_accepts_an_already_completed_predecessor_at_final_inventory`
   injects old Completed at the same boundary, completes rotation and repeats it;
   both Waves retain the intended current Projects and transferred Tasks.

The shared `local_started_task` SQLite reread and all existing archived-history,
legacy-adoption and twelve-mutation recovery fixtures are preserved verbatim.

## Checks and source boundary

- `git apply --check --whitespace=error scratch/parallel-h7-repair.patch`: pass.
- `rustfmt --edition 2021 --config skip_children=true --check` on all four
  private patched Rust copies: pass. This parses/formats; it does not type-check.
- New `RenameProject` operation validates against the supervisor's retained
  official Linear schema using cached `graphql-core`: zero errors, no network.
- Diff: **227 additions / 14 deletions**, including 182 additions / one deletion in tests and fixtures.
  Production: 45 additions / 13 deletions. No unrelated formatting hunks.

Base and final inspected HEAD: `d07e569330c8dedceb5dd238ce9b22a7b6137006`.
All four source hashes were unchanged between capture and final applicability
check. The base includes the main worker's existing dirty fixture correction;
this is a patch against working bytes, not a clean-HEAD diff.

| Source path under `rust/loopflow/src/` | Base/final SHA-256 |
| --- | --- |
| `ops/pm.rs` | `3ec2dfd7343f94b03c6fda98224972afe8c1e51920da8dca28ea74cc47264ff3` |
| `pm/linear.rs` | `05f7c4a8b7d3b6293fbbaec96833d9affd61a24a1f62d33362a1e1c5abb6097e` |
| `ops/chapter.rs` | `459300946a8bfe786bb34d8a463d62a5e582d51b60d20a82bd0eb920d6a48187` |
| `ops/chapter_tests.rs` | `fc7e81a593fa399e47c5ae1d2f8f2d05fc9b4355fee2513e4ab7e4b067fa9ee1` |

Patch SHA-256: `ea77b0d35f74157b430472701033520cbea60b978574fa53de2b3ceb8d17900b`.
Private originals/patched copies, source hashes and GraphQL receipt:
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-h7-repair-2wc4k814/`.
`final.json` includes each patched hash; `graphql-validation.json` includes the
schema hash and exact query. Recheck applicability if shared source changes.

## Main-worker verification

After integration, use the sole build slot and existing bounded runner. It
scrubs inherited LF/LOOPFLOW authority, supplies a disposable Home/DB, sets
four Cargo jobs, runs at nice +10 and enforces 900 seconds. No duplicate
Nextest `-j`/`--test-threads` flags:

```sh
uv run --no-project python scripts/resource_envelope.py
uv run --no-project python .lf/tmp/cut-i/run.py h7-repair cargo nextest run -p loopflow --lib --test-threads 4 --no-fail-fast -E 'test(explicit_sync_renames_legacy_projects_without_rewriting_authored_content) | test(rotation_preserves_conflicting_statuses_observed_in_the_final_inventory) | test(rotation_accepts_an_already_completed_predecessor_at_final_inventory)'
```

Include retained H7 preservation/retry coverage once in the integrated batch
(or reuse an identical-tree result):

```sh
uv run --no-project python .lf/tmp/cut-i/run.py h7-preservation cargo nextest run -p loopflow --lib --test-threads 4 --no-fail-fast -E 'test(every_provider_mutation_recovers_on_the_same_or_a_second_home) | test(legacy_project_adoption_preserves_plans_across_lost_responses) | test(archived_predecessor_is_history_even_when_linear_still_says_started) | test(partial_status_flips_converge_without_selecting_a_newest_name)'
```

Whole-tree formatting/Clippy belong to the main worker's integrated checks;
none ran here. There is no schema change requiring a new materialization proof.

## Review and limits

The patch handles observed conflicts; it cannot make a Linear read and later
write atomic. Another writer can still change status after the final read.
Treating this as a distributed concurrency guarantee would be unsafe. Name-only
rename preserves other fields even when their content changes concurrently.
No rollback is attempted for Task moves/cancellations already confirmed before
a status conflict. The selected status remains visible for reconciliation.

The fixtures model provider responses; they are not configured Linear, separate
live Homes, Desktop or whole-Task acceptance. Syntax and GraphQL validation do
not establish that these Rust tests compile or pass. No new product decision is
required for these two repairs.
