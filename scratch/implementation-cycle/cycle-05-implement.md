# Cycle 5 — implement: Task Comments

2026-09-25. A bounded child contribution. Jack requested collapsed Comments with
the real count below Description (accepted design, `main-view-task.md`).
Parent owns integration, compress, review and demo. Nothing was committed,
published, installed or written to PM, and no Session was touched.

## Result

- **Shared read.** Added `lf pm task comments --id ISSUE [--wave W] [--json]`
  (`ops::pm::pm_task_comments_async`). It uses the existing boundaries:
  - repository PM context, including the Team claim;
  - `resolve_owned_issue`, which checks the issue belongs to the named Wave and
    resolves its UUID;
  - the existing paginated `LinearClient::observe_issue`.

  It is read-only. It writes no Task preparation, no Started event, no Work
  registration and no snapshot, so an unprepared planning Task can use it.
- **Wire type** `TaskComments { wave, issue_id, identifier, observed_at, comments }`.
  Each comment has `id`, `body`, `author` and `created_at: Option`. The author is
  tagged `{kind: person, name}` or `{kind: integration}`.
  - There is no count field; the count is `comments.len()`. A partial read is an
    error, never a shorter list.
  - Rust, Swift and `tests/fixtures/dto/task_comments.json` move together. Swift
    requires every key; `null` stands for "absent".
- **Authorship keeps direction semantics.** A person is a comment with a provider
  `author_id`. A comment with no user is an integration, whatever its display.
  - An explicit `loopflow-steer` comment counts as a person, matching
    `is_direction_comment`. Its name comes from the requester marker, via the new
    shared `linear_observe::comment_requester`, which `render_comment` now also
    uses.
  - `is_human_comment` and direction ingestion are unchanged.
- **Provider reader.** Both comment queries now request `createdAt`, and
  `IssueComment.created_at` is added. Direction still orders comments by
  revision; the display read orders by `created_at`, then id. Missing
  continuation cursors still fail.
- **Swift.**
  - `RegistryQuery.taskComments` does the transport.
  - `PodiumModel` keeps readings keyed by planning Task, with per-Task
    generations and in-flight state. Keying by Task means a late response can
    only settle its own Task, and an older generation cannot replace a newer one.
  - On failure, the last good thread is kept. A cancellation caused by leaving
    the Task does not count as a failure.
  - `WorkspaceNavigation.expandedComments` is presentation state for each
    repository and window.
- **View.** `TaskCommentsView` sits below Description.
  - Collapsed, it shows **Comments (n)**. Before the first read it shows
    **Comments** with a spinner.
  - Expanding rereads the thread. Each comment shows the author ("Unnamed
    person" or "Integration"), a formatted date or "Date unavailable", and a
    Markdown body through the shared `MarkdownBlocks`.
  - A failed read shows **May be out of date** / **Unavailable**, the error and
    **Retry**, and keeps the earlier thread.
  - Loopflow's HTML-comment markers are hidden in display only; the stored body
    is unchanged. An empty body reads "Empty comment".
  - The thread is read when the Task is shown, and again on expand or Retry.
    There is no poller, and other Tasks' threads are not fetched.

## Proof

Commands cleared the ambient `LF_*` Home, Run and Wave variables.

- `cargo test -p loopflow --lib -- ops::pm::task_comments_tests pm::linear::tests::observe_issue ops::linear_observe`
  → **10 passed** (`/tmp/loo291-c5-rust-final.log`). The new
  `task_comments_read_every_page_in_written_order_without_writes` test calls the
  real ops function against a local axum Linear GraphQL server, which covers:
  - Team claim, issue ownership and a three-page thread across two
    continuations.
  - A comment with no `createdAt`, no body and a null user, which becomes an
    integration comment. A person with a null display name is also covered.
  - A steer published through an integration that names its requester.
  - Ordering by creation time, not revision; fixture revisions are identical, so
    the re-sort is exercised.
  - Nested list, link and code bodies preserved exactly.
  - A wrong-Wave rejection, an empty thread, a missing continuation cursor
    (error) and a GraphQL error.
  - Zero mutation queries, and no Wave or Work registered.

  This is the closest local path. The CLI binary can only reach a fixture URL
  under `cfg(test)`, so the binary itself was checked only through `--help`
  parsing.
- `cargo test -p loopflow --test dto_fixtures` → **9 passed**. The new fixture
  round-trips, and a comment missing `author` is rejected.
- `cargo fmt --all --check` and `cargo clippy --all-targets -- -D warnings`
  pass (`/tmp/loo291-c5-clippy.log`). `git diff --check` passes.
- Swift:
  `swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel --filter 'TaskCommentsProofTests|TaskFlowProofTests|WorkspaceNavigationTests|WorkspaceNavigationProofTests/namedSessionDrillDownRetainsTerminal|DTOFixtureTests'`
  → **42 tests / 6 suites pass** (`/tmp/loo291-c5-swift-final.log`).
  `commentsFollowTheirTask` mounts the production `SessionsView` with real
  Ghostty surfaces and two `/bin/cat` PTYs. It checks:
  - The exact `pm task comments --id issue-review --wave product --json` argv.
  - The collapsed count label and no thread.
  - After expanding: authors, the nested Markdown list, "Integration" /
    "Date unavailable" and "Unnamed person".
  - A failed refresh keeps the thread, marks it stale and shows the reason.
  - A late read of Task A is held while Task B is selected. B shows `0,
    collapsed`, and releasing A settles only A. Expansion is per Task.
  - Returning to A shows the fresh count and clears the stale flag.
  - No non-read operation occurs.
  - The layout is unchanged, the surfaces are identical, and the Session draft
    and companion PTY reply.
- **Counterexample.** Replacing the last-good retention with `lastGood: nil`
  makes the native test fail on the stale/thread assertions
  (`/tmp/loo291-c5-swift-falsify.log`). Retention was then restored.
- **Earlier failure.** The first Rust DTO attempt asserted that a missing
  `Option` field is rejected. Serde defaults `Option` to `None`, which matches
  the repo's other Rust DTOs, and only Swift enforces the key. The Rust check now
  removes a required field. The Swift test still requires `created_at` to be
  present.
- **Capture:** [task-comments-expanded.png](cycle-05-evidence/task-comments-expanded.png).
  This is fixture planning and comments, with real PTYs in a hidden window. It
  is not configured data. The capture shows the Flow slice's scalar "Loop ·
  Iteration 3" label. That label is the tuple owner's pending correction, not
  part of this slice.

SHA-256 (prefix):

```text
ops/pm.rs 66e61c56f150b873   ops/pm/task_comments_tests.rs d3ca49e067007d45
pm/linear.rs dc1f06780aa5a3ca pm/mod.rs 51089d69e4e334df
ops/linear_observe.rs b9db88d8c0693d5e lf/mod.rs 634d17bfe5233b7a
lf/commands/ops/mod.rs c3ab7f892b6581d6 tests/dto_fixtures.rs f2b78577fe69f0fd
task_comments.json ebf3e80ea993992f TaskComments.swift 8dab1502e4772c66
RegistryQuery.swift c8c5b4fcf0f89753 PodiumModel.swift fb5be69383511137
WorkspaceProjection.swift 732ded5a6ad73ca0 TaskCommentsView.swift c643d166ac0d74d9
WorkSurfaceView.swift 793e14356d2f9018 TaskCommentsProofTests.swift 196e2248b582ef6e
DTOFixtureTests.swift 37ef6d4512e880d9
```

Several of these files also hold other writers' concurrent edits, so the hashes
identify an observed state, not this contribution alone.

## Ownership and deletions

- **One reader.** The `observe_issue` pagination serves direction ingestion,
  reteam and now display. The new ops function only adds a projection.
- **One requester rule.** It moved out of `render_comment` into
  `comment_requester`.
- **No new store.** There is no comments cache, registry, writer or poller.
  Podium's per-Task reading is presentation lifetime, not authority.
- **Deleted.** The candidate `updated_at` display field was removed before
  finishing, because the UI consumed nothing from it.

## Notes for parent

- **`docs/lf.md` is yours.** I did not add the command example. Proposed line,
  after `lf pm task update`: `lf pm task comments --id 1207... --json   # read the thread`.
  `swift/README.md` documents it.
- **`cargo fmt --all` ran.** If it reformatted concurrent writers' files, the
  changes are formatting only.
- **`FlowSource` in `TaskFlowProofTests` does not answer `pm task comments`.**
  Its Task overview therefore shows Comments as unavailable. No assertion
  depends on that.

## Remaining gaps

- There is no configured-app read against the real Linear account. `lf pm task
  comments` was never run against live Linear.
- Integration (bot) authors have no display name. The query does not request
  `botActor`, to avoid changing the shared direction query's schema without
  verification.
- Posting and editing comments are out of scope.
- The rest of the Task evidence work is still open:
  - recent Runs disclosure;
  - provider/useful Session summary;
  - membership-chip graph navigation;
  - real delayed New-session preparation proof.
- Other open work: tuple iteration (owned by the control conversation), the
  configured demo, measurements, external trials and the authorized edit.
