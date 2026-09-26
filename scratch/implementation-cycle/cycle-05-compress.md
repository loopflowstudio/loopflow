# Cycle 5 — compress: Task Comments

2026-09-25. Two small reductions in the Comments contribution. The migration
adoption was inspected and left unchanged. Nothing was committed, published,
installed, or written to PM or a live store.

## Model before → after

Path traced: `LinearClient::observe_issue` (existing pagination, unchanged) →
`ops::pm::pm_task_comments_async` → `TaskComments` wire type → `lf pm task comments
--json` → `RegistryQuery.taskComments` → Podium per-Task reading → `TaskCommentsView`
→ shared `MarkdownBlocks`.

1. **The wire envelope echoed the request.** Before, it was
   `TaskComments { wave, issue_id, identifier, observed_at, comments }`.
   - No consumer read `wave` or `observed_at`.
   - `issue_id` existed only for a Swift guard that the response named the issue
     it was asked for. That is a check against an impossible CLI mismatch.
   - After: `TaskComments { identifier, comments }`. `identifier` stays because
     the CLI text output uses it, and it is the readable key when a caller passes
     a UUID.
   - Deleted the Swift `issueId` guard and its `RegistryQueryError`. Rust, Swift,
     `task_comments.json`, the DTO tests and the native proof's fixture reply all
     changed together. Every field is still required; Swift still requires
     `created_at` to be present as `null`.
2. **The steer marker was written out three times.** `is_direction_comment`,
   `comment_requester` and the new display classification each wrote
   `body.contains("<!-- loopflow-steer:")`. Added `linear_observe::is_steer`, and all
   three now use it.
   - The display rule (a person is anyone with a provider user, or an explicit
     steer) is still separate from direction eligibility (`is_direction_comment`,
     which excludes Loopflow writebacks). Authorship display does not change
     which comments count as steering.

## Retained deliberately

- `commentsInFlight` is kept separate from `PodiumReading`. A refresh while the
  last good thread is showing is a different fact from "never read".
- Per-Task generations and the cancel-is-not-failure rule are kept. The late-read
  invalidation proof depends on them.
- `expandedComments` stays on `WorkspaceNavigation`, because it is presentation
  state for one window/repo.
- `readableBody` HTML-comment stripping happens only when displaying; the stored
  body is untouched. There is one Markdown parser and renderer (`MarkdownBlocks`).
- The CLI text renderer's author wording repeats Swift's labels only across the
  language boundary. It is not duplicate authority.
- **Migration adoption** (`adopt_released_development_drafts`): one owner inside
  `apply_installed_development_sqlite`. It parses release `-- draft:` provenance.
  The other matches in `migrations.rs` are test helpers and `migration_sql_for_test`,
  which only locate a marker. No production parser is duplicated. Real chapter data
  is preserved and nothing is replayed. No change.

## Proof

Ambient `LF_*` Home/Run variables were cleared.

- `cargo test -p loopflow --lib -- ops::pm::task_comments_tests ops::linear_observe pm::linear::tests::observe_issue`:
  **10 passed**.
- `cargo test -p loopflow --test dto_fixtures`: **9 passed**.
- `cargo fmt --all --check` passes. Earlier, `cargo fmt --all` ran first; if it
  touched other writers' files, the changes are formatting only.
  `cargo clippy --all-targets -- -D warnings` passes, and `git diff --check`
  passes for rust, swift and tests.
- `swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel --filter 'TaskCommentsProofTests|DTOFixtureTests/taskCommentsFixture'`:
  **2 tests in 2 suites passed**. This includes the native late-read, stale and
  Session-retention proof (`/tmp/loo291-c5c-swift.log`).
- No failures occurred. Migration tests were not rerun because `migrations.rs` is
  unchanged by this pass.

Pre-edit copies are in `/tmp/loo291-c5c-before/`. SHA-256 prefixes:

```text
ops/pm.rs 851698474122776f              ops/linear_observe.rs 11ab6e1b39550f5c
ops/pm/task_comments_tests.rs 25dacbbbff792d31
task_comments.json 6d9c65024674da2d     TaskComments.swift 64aae0541ada7c6c
PodiumModel.swift adcb0cc75983a656      DTOFixtureTests.swift f57f175b1bc32a81
TaskCommentsProofTests.swift 4dcc1be3282ac627
store/migrations.rs f2e74f903047ae38 (unchanged)
```

`PodiumModel.swift` and the Rust files also hold other writers' edits, so these
hashes identify the state observed here, not this pass alone.
`cycle-05-implement.md` still describes the old five-field wire type; this receipt
supersedes that description.

## Gaps (unchanged)

- `lf pm task comments` has not been run in the configured app against live
  Linear. Integration authors have no display name.
- Remaining Task evidence:
  - recent Runs disclosure;
  - provider/useful Session summary;
  - exact membership-chip graph navigation;
  - real delayed New-session preparation proof. Task-keyed feedback is already
    implemented.
- Other open work:
  - the tuple iteration correction, owned by the control conversation;
  - final native build and configured Home readback;
  - the demo, measurements, external trials and the authorized edit.
