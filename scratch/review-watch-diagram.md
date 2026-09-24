# Connected Watch diagram review — 2026-09-24

Disposition: the connected-diagram slice passes its local acceptance claims.
Refresh PR #1276 as an in-progress Task. The configured Watch demonstration and
human gate remain required before settlement; this is not landing approval.

Read the Task directive, current slice and complete target in
`restore-task-watching-with-live.md`, the diagram implementation, its full working
delta and untracked evidence, and the prior reader/feed/navigation reviews.
Compared the complete base-to-worktree inventory with those reviewed foundations.
The inherited LOO-291 and Rust receipts retain their original scope; this review
does not claim to rerun or newly validate that entire inherited change.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Exact stage selection | Custom/repeated names identify distinct stages | Invocation plus step index selects and filters existing output | `diagramNavigation` taps actual diagram button actions; Store source | pass, local |
| Recorded returns and retries | Iterate targets and separate attempts survive | Solid paths use retained edges; links select exact destinations; attempt detail retains Run identities | Same test checks Iterate/retry and three distinct attempts; inspected full diagram | pass, local |
| Honest historical state | Completion never invents entered/completed stages | Settlement and last recorded attempt remain separate; empty plans stay explicit | Completed and missing-plan cases in `diagramNavigation`; stale-selection proof | pass, local |
| Connected native layout | Diagram beside attempts, feed below | Native measured stage bounds, ancestry headings, dashed plan order and solid transitions | Full diagram and integrated NSWindow renders inspected | pass, rendered fixture |
| Keyboard and motion | Select adjacent expanded stages without animation | Native buttons and Up/Down callbacks; selection scrolls without animation | Callback proof and source; physical keyboard/scroll trial absent | pass locally; configured input gap |
| Passive configured reading | Inspect without client takeover or worktree dependency | Read-only CLI resolves exact Task from `/tmp`; reports missing plan evidence | Fresh configured CLI receipt: 3 Runs, 0 invocations, `position_unavailable` | pass, historical read |
| Complete Watch | Live bounded feed, all capture, exact Session links and configured demo | Manual updates and several required gaps remain | Current design and reachable source | gap, required in this PR |

## Evidence and limits

The implementation receipt runs:

```sh
LF_WATCH_RENDER_PATH=/tmp/loo293-diagram.png swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'TaskWatchTests/(diagramNavigation|selectionAndStaleEvidence|renderSnapshot)'
```

Three tests pass, including two rendering cases; the Mac product compiles and
links. The diagram, Watch view, tests and log still match the supplied content
hashes, recorded in [review-hashes.json](watch-diagram-evidence/review-hashes.json).
No executable edits followed that receipt. This review reused the proof rather
than rerunning it solely because the lifecycle phase changed.

Inspected [the complete diagram](watch-diagram-evidence/plan.png) and
[the integrated workspace](watch-diagram-evidence/workspace.png). Nodes, ancestry,
retry/Iterate paths and attempt cards are legible. The integrated plan scrolls to
reach later stages; output and its manual-refresh explanation remain visible.
These are rendered fixtures, not configured live transition or physical keyboard
evidence. Repeated edges may share a lane; exact transition text remains available.

A fresh existing-branch-binary `lf task watch LOO-293 --json`, run from `/tmp`
against the explicitly selected configured Home/database, returned the exact Task,
three Runs and no retained invocation, with empty stderr and exit 0. The
[receipt](watch-diagram-evidence/configured-read.json) records time and binary
hash. This Home cannot demonstrate diagram transitions without manufacturing
production history. No provider, installed app or production history was changed.
The binary is an existing reader build, not a new current-head build.

The required Xcode compile path was attempted through
`uv run python scripts/test.py --loopflow`. Resource preflight stopped before any
suite ran: the active main build root uses 19.1 GiB against its 12.0 GiB budget.
[Receipt](watch-diagram-evidence/xcode-preflight.log). Earlier supported recovery
preserved that active root; this review did not bypass the envelope or remove it.
There is no Xcode compile verdict for this slice.

## Source review and next slice

Traced diagram actions through TaskWatchStore selection/filtering and retained
navigation, conditional Watch mounting, RegistryQuery's typed reads and Rust's
read-only Task lookup. The diagram consumes immutable snapshot stages and edges
directly. Node geometry is transient presentation, not another flow graph model.
The previous stage List and its active-stage helper are removed. Searches across
reachable Watch files find no provider launch/resume/replace/interrupt operation,
Session inventory, direct Swift provider-file/database reader, polling timer,
transcript writer or second lifecycle reducer. No wire contract changed.

No bounded executable defect was established. Corrected the design's stale
remaining-work list, which still described the implemented feed and diagram as
unbuilt. Also narrowed its geometry-reset wording: the current receipt does not
prove native focus reset across invocation changes.

Next: bound discovery/initialization and retained state before visible polling;
retain contiguous cross-Run observation order; finish autonomous/native capture
and configured provider proof; navigate to exact human checkpoint Sessions; then
perform the configured desktop scenario and human gate. Keep these obligations
in this same Task/PR. The connected view advances that architecture and introduces
no authority that the next slice must undo.
