# Cycle 5 — review-slice: Task Comments + demo-Home draft adoption

2026-09-25. Verdict: **both contributions advance the full design.** Neither needed
a product fix. One test assertion was strengthened: Rust's serialized Comments
output must equal the fixture Swift decodes. Comments is one audit item, not the
rest of the Task. Nothing was published, promoted or completed, and no live Home,
PM or Session was touched.

## Claim matrix — Comments

| Claim | Implemented | Proof | Result |
|---|---|---|---|
| Real, complete thread through the existing reader | `pm_task_comments_async` → Team claim → `resolve_owned_issue` (Wave ownership) → paginated `observe_issue` | `task_comments_read_every_page_in_written_order_without_writes` (real ops fn, local axum GraphQL, 3 pages / 2 cursors) | pass |
| Partial reads never become a shorter list | A missing continuation cursor is an error; a GraphQL error is an error | same test | pass |
| Written order, stable under edits | Sorted by `created_at`, then id; revision ignored for display | same test (identical revisions) | pass |
| Truthful authorship and dates | Person = has provider user, or explicit steer with its requester; otherwise Integration. `name`/`created_at` are explicit nulls; UI shows "Unnamed person" / "Date unavailable" | Rust test; native proof; fixture | pass |
| Read-only; unprepared Task gets no Work or Started | No store handle in the path; zero GraphQL mutations; no Wave or Work registered | Rust test asserts both | pass |
| Exact wire shape across the boundary | Required fields; Swift requires present nulls | **Strengthened here:** `to_value(thread) == fixture` in `dto_fixtures`; Swift `taskCommentsFixture` | pass |
| Collapsed `Comments (n)` below Description; expanded Markdown | `TaskCommentsView` after Description; shared `MarkdownBlocks` | Native proof; capture below | pass |
| Empty / failure / last-good / Retry | `No comments.` only after a successful read; a failure keeps the last thread with "May be out of date" and the reason; a first failure shows "Unavailable" and no count | Native proof | pass |
| Late Task-A read cannot publish into Task B | Readings keyed per Task with generations; leaving a Task cancels its read without counting a failure | Native proof: held A read, B shows `0, collapsed`, A settles only A | pass |
| Native continuity | Same surfaces, layout, draft and companion replies | Native proof (real Ghostty, two `/bin/cat` PTYs) | pass |
| One authority | One `RegistryQuery.taskComments` caller (Podium), one `observe_issue` reader; no cache, poller, writer or Description splitting | `rg` over Swift and Rust | pass |

Capture: [task-comments-expanded.png](cycle-05-review-evidence/task-comments-expanded.png).
It was inspected and shows:
- Comments (3) below Description.
- Polish direction D intact.
- Flow header `Iteration (1, 1)` with two labelled loops and the `then` tail. This
  comes from the parent's in-progress tuple work and is observed here, not reviewed.
- The Session row and New session are unchanged.

The capture uses fixture planning with real PTYs.

## Claim matrix — draft adoption (`demo-home-release-adoption.md`)

| Claim | Proof | Result |
|---|---|---|
| Released draft SQL is credited without being run again | `installed_development_home_keeps_chapters_when_its_draft_is_released`: the chapter receipt is byte-equal, the draft ledger empties, and a repeated apply leaves history unchanged | pass |
| Unreleased drafts keep their data, id and timestamp; positions renumber safely under UNIQUE | `installed_development_release_keeps_unreleased_drafts` (position 1→0, `applied_at` retained, row kept) | pass |
| Changed checksum rejects before the transaction; schema drift rejects inside it and rolls back | `…rejects_changed_evidence_without_losing_data`: history, drafts, schema and row count all unchanged | pass |
| Ordinary installed-dev opens only validate | `store/sqlite.rs:446` calls `validate_…` for installed development; apply runs only on the install/preflight path and private dev init | source | pass |
| Retained Home passes preflight on a copy | `/tmp/loo291-retained-home-preflight{,-after}.json` hashes match the receipt: before is `reject` (frontier changed before 0.12.21.001), after is `promote_and_migrate` with 32 compatible references | receipt | pass (not rerun) |

`migrations.rs` SHA-256 `f2e74f90…ce95` matches the receipt.

Notes:
- A pending release migration without draft provenance refuses rather than applying.
  This is conservative and safe; the recovery is a fresh fork.
- A packer that changes the trailing newlines of a draft gives a false refusal,
  never a false adoption.
- The pending set is read before `BEGIN EXCLUSIVE`. Promotion is single-writer, so
  this is accepted.

## Commands

The following commands ran with ambient `LF_*` Home/Run variables cleared:

- `cargo test -p loopflow --lib -- ops::pm::task_comments_tests ops::linear_observe pm::linear::tests::observe_issue`
  → **10 passed** (`/tmp/loo291-c5r-rust.log`). It compiled against the parent's
  in-progress tuple source with no mismatch.
- `cargo test -p loopflow --test dto_fixtures task_comments` → **1 passed** with the
  strengthened assertion (`/tmp/loo291-c5r-dto.log`).
- `cargo test -p loopflow --lib -- store::migrations::tests::installed_development`
  → **5 passed** (`/tmp/loo291-c5r-migrations.log`).
- `LOOPFLOW_FLOW_CAPTURE_DIR=… swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel --filter 'TaskCommentsProofTests|DTOFixtureTests/taskCommentsFixture'`
  → **2 tests / 2 suites passed** (`/tmp/loo291-c5r-swift.log`).
- `rustfmt --check` on the edited file, `cargo clippy --all-targets -- -D warnings`,
  and `git diff --check` all pass.

There were no failures.

SHA-256 prefixes at review:

```text
ops/pm.rs 851698474122776f            ops/pm/task_comments_tests.rs 25dacbbbff792d31
pm/linear.rs dc1f06780aa5a3ca         ops/linear_observe.rs 11ab6e1b39550f5c
tests/dto_fixtures.rs 29370bc08efe31ee store/migrations.rs f2e74f903047ae38
task_comments.json 6d9c65024674da2d   TaskComments.swift 64aae0541ada7c6c
TaskCommentsView.swift c643d166ac0d74d9 PodiumModel.swift adcb0cc75983a656
RegistryQuery.swift c8c5b4fcf0f89753  TaskCommentsProofTests.swift 4dcc1be3282ac627
```

Several of these files also contain the parent's concurrent tuple and polish edits,
so the hashes identify an observed state.

## Limits and remaining gaps

- The **real `lf` binary against an HTTP fixture** is not exercised. The GraphQL URL
  override is `cfg(test)`-only, and adding a production seam for tests is forbidden.
  The closest local path is the ops function plus exact argv (Swift) and exact wire
  shape (Rust). The configured live Linear read in the app remains open.
- **Cancellation of a structured read mid-flight** is covered by source only. The
  proof holds an unstructured Retry read.
- **Sort order:** it compares ISO strings, which is correct for Linear's uniform
  `.000Z`. Mixed fractional precision within the same second would misorder.
- **Integration author names:** `botActor` is not requested.
- **Review scope:** this review read the Comments and migration diffs in full, and
  the rest of the Task only where it touches Comments. Earlier cycle receipts cover
  the other historical changes, which were not re-reviewed.

Still open from the audit:
- Recent Runs disclosure.
- A provider/useful Session summary.
- Membership-chip → graph node navigation.
- Real delayed New-session preparation proof.
- The tuple correction's own review.
- Promotion and readback of the retained Home, which the parent owns.
- The configured demo, measurements, external trials and the authorized Description edit.
