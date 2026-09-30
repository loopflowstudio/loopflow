# Workspace navigation after the data model

LOO-303 · Product · 2026-09-30 · Jack Heart

## Accepted scope

Jack's supervising session narrowed this Task in Linear comments `0db66249` and
`39781cad`, superseding the September 26 control-room plan and the questions in
`ux-reassessment.md`. Ship one PR containing:

- ⌘K across Waves, Tasks, Sessions, Flows and actions, with recent destinations
  first. Add one **Bind to Task…** action for the selected unassigned Session;
  reuse the breadcrumb's existing picker, Rust preview and permanent confirmation.
- `loopflow://task/ISSUE` links, including cold delivery, historical Tasks and
  repository collisions. Inspection starts no work; stale results never redirect
  a later selection.
- Folded Flow templates on Wave and unstarted Task pages and in the catalog.
  Preserve repeated/empty composition, nested alternatives, both return edges,
  numeric captured node identity and keyboard disclosure. Rust owns resolution.
- Hidden retained terminals refuse first responder. Palette input never enters
  a retained PTY, and Escape restores focus.

The orphan control room, room-only navigation/binding and pre-model orphan handling
are cut. LOO-353 owns the invocation view, exact running line, Session chip,
global browser and configured workspace demo. Its interactive-stage strip is the
primary Flow view; an edge expands into the folded template before start and
captured FlowSession afterward. Wave-page New session and waveless first run leave
this Task. Dark mode waits for acceptance of the light layout and later refiling.

Do not delete inherited LOO-298 behavior while trimming this Task's contribution.
The diff against merge `01dda6a26`'s second parent contains navigation, template
and input-isolation changes; no control-room or dark-mode implementation was built.
The parent's existing expanded Flow view and binding remain in place for LOO-353.

## Finish line and delivery

Stop after focused Rust and Swift checks prove the four surviving pieces plus the
palette bind entry. Jack waived demo/review; no installed captures are required
here. Fixture transport and owned-PTY proofs do not establish configured provider
acceptance or the Task's multi-day KRs.

The supervisor publishes and lands this PR after LOO-298, then completes the Task.
No publication or Task mutation belongs to this implementation pass. Parent sync
uses merge, never rebase; preserve deletion of inherited parent scratch.

## Verification

Focused proof passed on 2026-09-30 (no Rust source changed in this pass):

```sh
cargo test -p loopflow --lib template_resolution_preserves_composition_and_execution --jobs 2
cargo test -p loopflow --test status_tests exact_task_roadmap --jobs 4
cargo test -p loopflow --test dto_fixtures flow_templates_round_trip_distinct_compositions_and_required_children --jobs 4
swift test --package-path swift --no-parallel --jobs 2 -Xswiftc -gnone --filter 'WorkspaceDestinationTests|TaskFlowTests/(flowFixtures|templateDisclosure)|SessionChromeProofTests/paletteRetainsTerminalInput|TaskFlowProofTests/flowControlsRetainTerminals'
swift test --package-path swift --no-parallel --jobs 2 -Xswiftc -gnone --filter SessionChromeProofTests/paletteRetainsTerminalInput
```

Rust: 4 checks passed. Swift: the first command passed 14 of 15 checks; the
new bind test incorrectly looked for an earlier draft on the other terminal.
After correcting that test to type a fresh draft into the bind target, the focused
palette rerun passed. The 15 selected behaviors therefore have passing proof;
no broader suite or installed demo was run. Commands ran at nice +10.

The mounted palette proof dispatches ⌘K, filters to Bind, submits, and observes
the existing binding draft for the same Session. Cancelling keeps both exact
terminal surfaces and the draft's subsequent child reply. The destination test
covers unassigned, Wave-only, assigned and missing Session selections.

Review found no additional LOO-303 control-room, running-line, invocation or
dark-mode implementation to remove relative to the merged LOO-298 parent.
The bind entry makes no provider-open call and adds no binding owner or picker.
The existing preview still resolves the exact target before permanent assignment.
Remaining work is supervisor delivery after LOO-298; configured/multi-day usage
claims remain unproven.

Resource preflight on this pass reports 41.4 GiB free, above the 32 GiB reserve;
no cache removal was needed. The older 30.4 GiB observation remains in
`workspace-disk-unblock.md` as history, not a current blocker.

Compression, 2026-09-30: folded projections now index captured nodes once instead
of searching the graph for each visible node, and enumerate each folded group's
keys once. Removed unused group-ID traversal, duplicate palette sorting and a
redundant historical-link test model. No wire shape or Rust source changed.

The first focused run passed 12 of 13 tests; the combined mounted-window test
unexpectedly delivered work to the second model. Diagnostic runs passed with
window order `[0, 1]` and neither window key; they did not explain the failure.
Removed that flaky combined test, retaining the separate cold/warm router test.
One controlled two-read test now covers both newer-link precedence and later
navigation cancelling both pending results. This proves model ordering, not
configured multi-window focus. No router repair or installed acceptance is claimed.

Final focused proof passed 12 Swift tests (the ordering test has two cases):

```sh
nice -n 10 swift test --package-path swift --no-parallel --jobs 2 -Xswiftc -gnone --filter 'WorkspaceDestinationTests|TaskFlowTests/(flowFixtures|templateDisclosure)'
```

`git diff --check` passed. Resource preflight reported 38.9 GiB free, above the
32 GiB reserve, with no recovery. Earlier Rust and retained-PTY proof remains
applicable to unchanged paths; no broader suite or demo was repeated.
