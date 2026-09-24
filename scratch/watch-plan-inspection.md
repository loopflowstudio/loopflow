# Watch plan inspection — 2026-09-23

This slice makes the shared snapshot visible. The next implementation needs
bounded discovery and independent output history/live cursors before polling,
then the feed, filters/Follow live, Session navigation, and configured demo.
Manual plan inspection does not satisfy the complete Task.

## Review and proof

| Claim | Implementation / proof | Result |
| --- | --- | --- |
| Inspect without a worker/worktree | Watch sits outside the workspace prerequisite; the query has nil cwd. A focused view test finds Watch with runtime/workspace absent. | pass |
| All Task entry points | Watch buttons open the existing workspace sheets in Work, Roadmap and Wave detail. Source inspection and Mac compilation; no hosted clicks. | local proof |
| Exact plan/stage identity | Rust DTO is retained as-is; Swift selection is invocation ID plus step index. Repeated-name test retains the chosen human stage. | pass |
| Attempts and Iterate | View shows separate Run IDs, recorded state/readiness/failure and exact transition targets. Fixture test checks the Iterate control and return selection. | pass |
| Historical inspection | Refresh/completion retain selection; earlier invocation remains selectable. | pass |
| Honest failed reads | Last snapshot stays visible under a stale banner, successful refresh clears it. Old finishing reads cannot replace newer evidence/errors. | pass |
| Passive visibility | New Watch path calls taskWatch only. Shared query runner cancels its own subprocess on view-task cancellation. Real sleep subprocess test proves cancellation before and after spawn. | pass locally |
| Keyboard / reduced motion | Native Picker, List selection and Buttons; no explicit animation or motion. Keyboard event walkthrough and accessibility audit remain unperformed. | source proof |
| Visual surface | Window-backed AppKit rendering of the real view with the shared fixture inspected at 1000×720. | fixture proof |
| Live output / complete capture / human demo | Not implemented by this slice. | gap |

The first offscreen render lacked native AppKit controls and had a transparent
background. It was not accepted as visual proof. The final renderer uses an
unpresented NSWindow, and Watch supplies its palette background. One render
build stopped because RoadmapView was edited while Swift was compiling it;
the subsequent stable-source focused command passed. No production state or
provider was changed to manufacture evidence.

Final focused command:

```sh
LF_WATCH_RENDER_PATH=/tmp/loo293-watch-snapshot.png swift test --package-path swift --filter TaskWatchTests
uv run python scripts/check_swift_multiplatform_boundaries.py
```

Five tests passed; SwiftPM built and linked the Mac target. The shared fixture
test also passed before the new UI tests were added. Existing Ghostty linker
symbol warnings remain. The Xcode hosted configuration and full suite were not
run. Whitespace check passed. No Rust code changed in this slice.

## Ownership and limits

TaskWatchStore holds snapshot/selection/request state, not a persisted cache or
flow reducer. No YAML reader, ledger writer, new DTO, native source reader,
provider launch/resume, Session action, or polling timer was introduced. The
shared query runner still owns process setup, pipe draining, and error handling;
its cancellation receipt owns only that query child and fences cancellation
against spawn. Task execution and provider-client authority remain untouched.

Watch intentionally does not infer liveness from `bound` or completion from a
missing active stage. Source capture gaps remain owned by TaskOutputPage. Human
checkpoint selection currently inspects retained facts; links to the existing
Session controls remain required alongside the future feed.

This checkout is shared. Concurrent README, compression/review notes and the
hosted launcher call-site update were preserved. This run made no commit or
publication and did not claim those unrelated edits.

Implementation cross-check: the hosted launcher test now awaits the async query
API. SwiftPM excludes that hosted-only branch, so its Xcode compilation remains
unproved. A subsequent focused Watch run passed all five tests, the Mac product
build passed, and the window-backed PNG was visually inspected with its native
stage list and invocation selector present. Whitespace checking passed. These
are shared-working-tree receipts, not configured UI interaction or a broad gate.


## Rebase onto main-view-task — 2026-09-23

User-requested integration uses `jack-heart/main-view-task` at `9b3264efd`.
`lf rebase --manual` replayed the local branch after checkpointing all prior work.
Generic scratch-note name conflicts retain this Task's current notes followed by
clearly labeled LOO-291 evidence. WorkSurfaceView keeps the target's unified
inspector routing, restoring only the Watch sheet, its selection state and shared
terminal store. No removed root navigation was reintroduced.

The first post-rebase focused build caught the target's removal of
WorkTaskSelection's Identifiable conformance. Restoring its existing Wave/Task
identity fixes sheet presentation. The final `swift test --package-path swift
--filter TaskWatchTests` passed all five tests and built the Mac target. This is
local integration proof, not a configured live demo. No push was performed.
