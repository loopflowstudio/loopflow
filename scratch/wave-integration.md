# Wave-only workspace integration — 2026-09-24

## Result and remaining work

Public navigation is repo → Wave → Task → Session. Compact and flat Sessions
use the same outline; Full no longer introduces a Project tier. Wave details
retain objective, current chapter plan/KRs, all Tasks and history. Session,
Monitor and ordinary terminal panes retain the existing multiplexer and native
surface owner. Required Session-to-Run identity is unchanged.

Integrated committed `loopflow.projects` snapshot `48622b569` through local
`lf rebase --manual`, after checkpoint `bd59cdda4` (rebased to `0120c0757`).
The sibling's uncommitted ownership correction was not copied. Its source
snapshot still has the recorded metric-target/objective correction outstanding.
No live chapter rotation, PM mutation, installed-app replacement, publication,
worktree closure or Task completion occurred.

Remaining: reconcile that ownership correction when committed; finish bounded
active discovery/shared refresh and both native performance experiences, fallback
compilation, configured positive activity and human composition acceptance.
Preserve the original external-work/edit proof obligations and one optimization
Task per measured area. LOO-293 remains open.

## Reconciled ownership

- Rust chapter binding and direct-Task status/roadmap/PM reads come from the
  committed sibling. Current/unavailable Task evidence stays shared; no Swift
  Project-array adapter or second chapter pointer.
- Deleted `WorkspaceProject`, Project compression/disclosure, Project inspector
  and ordinary Project conversation scope. Kept this branch's current outline,
  scoped conversations, Session actions, retained workspace and Monitor. The
  sibling's older PodiumConsole/workspace was not restored.
- Current Project reference links resolve to their Wave; historical references
  open chapter history. Project-bound historical Sessions use shared `wave_id`,
  not a join against only the current Project. Missing ancestry stays reachable.
  The shared Run resolver reads historical Project slugs/provider IDs through
  the existing stored Project lookup; removing ordinary Project launch selectors
  must not prevent reading these prior Runs. The ancestry regression covers both
  declared forms without launching a provider.
- Shared Session display paths now contain Wave/Task. Wave ancestry is projected
  consistently when listing or opening Ask/interactive Sessions; removed the
  list-only enrichment that could disappear after opening. Original typed Work
  and Run provenance remain intact.
- Task activity is filtered by stable Task identity, without its old Project.
  Selected Task evidence during a transfer lives in existing repository/window
  navigation, alongside selection and saved panes. It is retained reading
  evidence, not another planning writer. A complete new plan still reconciles
  genuinely absent work.

## Verification

| Behavior | Proof | Result |
|---|---|---|
| Direct navigation and compression | Full/compact/Session-leaf identity, empty/partial/unavailable, upcoming work and exact Session actions | pass |
| Chapter transfer preserves useful workspace | Mounted production SessionsView; transition hides then restores Task membership; same Monitor pane, Task Work, Session Run and native surfaces | pass, fixture transport |
| Retained input and companions | Original cat PTYs retain exact draft and respond after chapter transfer, zoom, resize and Session return | pass, native owned PTYs |
| Details and history | Current chapter references route to Wave; directive editing/read generation, unavailable Task diagnostics and Task Activity remain valid | pass, focused model/view |
| Rust contracts | Eight DTO checks; historical Project ancestry maps to Wave, no public Project display tier | pass, local store/fixtures |
| Static checks | Native app compilation, Rust all-target compilation, cargo fmt and clippy with warnings denied | pass |

Swift command:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel \
  --filter 'WorkspaceNavigationTests|TaskMonitorProofTests|TaskMonitorTests|PodiumModelTests|DTOFixtureTests|TaskDirectiveEditorTests|WorktreeWorkspaceTests'
```

62 tests pass in [the combined log](wave-integration-evidence/swift.log).
The subsequent [native transfer extension](wave-integration-evidence/transfer.log)
also changes each Task's runtime parent/routing Project and passes separately
(one test). The two [Swift Session contract checks](wave-integration-evidence/session-swift.log)
pass against the corrected shared UUID fixture.
Rust [DTO](wave-integration-evidence/dto.log),
[ancestry](wave-integration-evidence/ancestry.log),
[Session actions](wave-integration-evidence/actions.log) and
[clippy](wave-integration-evidence/clippy.log) receipts retain their commands'
output. [Source hashes](wave-integration-evidence/sources.json) identify the
reconciled producers and consumers. These are local behavioral and static
proofs; no configured migration, provider trial or performance result is implied.

Counterexamples retained: the first Swift build exposed stale Project types;
the first focused compile found stale test fixtures; a subsequent compile was
interrupted by a concurrent performance-test edit. The first completed test run
failed seven assertions because conflict resolution had incorrectly marked
absent fixture checkouts present. That changed legitimate workspace placement
and completed-Task membership. Restoring the original filesystem facts fixed
those failures; no production behavior was weakened to satisfy them. The Rust
Session action fixture then rejected the sibling's prefixed `wave_id`: Wave IDs
are UUIDs. Corrected both shared Session fixtures and verified Rust actions and
Swift round trips; [before](wave-integration-evidence/actions-before.log) and
[after](wave-integration-evidence/actions.log) are retained. Logs remain
under `/tmp/loo291-wave-integration-{swift,tests,tests2,tests3}.log`.

## Review and concurrent contribution

Negative searches find no `WorkspaceProject`, `RoadmapProject`, Project planning
DTO or PodiumConsole declaration in the reachable workspace. There is one Podium
caller each for roadmap, Sessions and active Runs, and one production root
workspace registry. The shared Work enum retains Project solely for historical
identity and references; it is not a public navigation or launch destination.

The performance contribution appeared after checkpoint and is preserved without
being claimed as this slice. Its scripts/tests and endpoint choices belong to
that writer. This integration only migrated its fixture to direct Tasks and
corrected the native accessibility protocol cast to unblock compilation; later
writer changes are preserved. Its opt-in benchmark was not run by this pass.
Do not infer painted-frame latency or optimization gains from the native test
suite's duration. No broad gate or fallback-preflight bypass was performed.
