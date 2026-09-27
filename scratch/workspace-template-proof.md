# Navigation repairs and folded templates

2026-09-27 · LOO-303 · implement

Jack's approved light-only scope and existing workspace composition remain the
base. This cut repairs the two concept-review findings, extends local navigation
proof, and implements the independent template slice. The complete
[design](workspace-ux-on-data-model.md) still governs parent integration and
configured acceptance. The supplied concept review was preserved first in
checkpoint `16bdd9f9e`.

## Behavior and ownership

- Palette highlighting and Return now share one selection drawn from the current
  results. Removal selects the first remaining row; an empty result has no action.
  Failed reads retain last-good rows with the existing stale presentation.
- Each repository/window retains at most 20 destination descriptors. A historical
  Task remains a recent destination after leaving its page and refreshing the
  current plan. Activation uses the existing exact reader and checks recorded
  Task identity and repository. Failure or an identity mismatch preserves the
  workspace; only a successful absent result removes that recent entry. Retry
  retains the expected identity. No second Task snapshot cache or all-history
  search was added; selectedTaskEvidence still owns the selected detail.
- The existing Rust resolution traversal now retains composition groups, including
  repeated and empty uses and every XOR alternative. One flattening function
  supplies execution steps. Catalog entries add a template tree referencing their
  existing graph and a digest of resolved content. Local group IDs are separate
  from invocation/node identity. Missing sources and cycles expose an unavailable
  entry instead of a partial executable graph.
- Task previews, catalog inspection and the Wave page's current Project template
  use the same disclosure view. Both return edges retain distinct labels when
  their endpoints fold into a group. Disclosure is repository-local and keyed by
  resolved revision. Group clicks and disclosure controls expand the same state;
  a changed revision starts folded. Captured invocations still render their
  captured graph independently of the catalog. Independent Task Runs do not
  manufacture an invocation.
- Source review found that chapter summaries returned a null Flow for a Project
  whose Tasks default to feature. The summary now uses the existing Rust Task
  default resolver. No authored Project value is changed and Swift invents no
  default. Wave presentation adds no execution control or non-Task inspector.

## Executed evidence

Logs are under `.lf/tmp/workspace-navigation/`. Product commands ran in isolated
LF authority with low priority, at most four aggregate build workers, serial
tests and a 900-second process-group timeout. No timeout fired. Resource preflight
passed before execution and again at 95.8 GiB free before Xcode compilation.
The final resource check also passes at 94.6 GiB free, with this checkout's
build outputs at 4.6 GiB of its 12 GiB allowance (`iterate-resource-final.log`).

| Check | Result and scope | Log |
| --- | --- | --- |
| Rust `--lib engine::flow` | 49 tests pass: existing resolution/policy/capture behavior plus composition, repeated/empty groups, XOR, both returns, content revision and unavailable source/cycle | `template-rust-fixed.log` |
| Rust `--test dto_fixtures` | 10 tests pass; catalog and richer template fixture round-trip; missing required children reject | `template-dto.log` |
| Actual CLI `status_tests::orphaned_task_work_preserves_status_and_roadmap_evidence` | 1 test passes; status/roadmap expose the same effective Project and Task default in an isolated Home | `template-project-default.log` |
| Swift destination, Session keyboard/PTY, Task Flow model and mounted Flow proof | 16 tests pass in four suites | `template-navigation-final.log` |
| Final Swift Task Flow model/native filter | 5 tests pass after group-click and revision-reset coverage and nested group click handling; 13.4 s including build | `template-path-final.log` |
| All-target Clippy, warnings denied | Pass, 20.1 s | `template-clippy.log` |
| XcodeGen and signed ad-hoc Xcode build-for-testing | Pass, 61.9 s; app, bundled CLI and test runners compile; hosted UI tests were not executed | `iterate-xcodegen.log`, `iterate-xcode.log` |
| Formatting, Swift platform boundaries, working diff whitespace | Pass | Command output |

The destination proofs include historical Task → Wave → successful refresh →
recent Task, repository isolation, failed/wrong-identity/missing readback, and
bounded recents. The two-window proof mounts production SessionsView and window
receivers, routes two held link reads into one window, invokes a mounted Wave
row's click, then releases the reads in reverse order. Neither late result
replaces the click or changes the other window. This is mounted routing proof,
not installed Launch Services delivery.

The dispatched AppKit palette proof uses two owned Ghostty cat PTYs with recorded
Session-to-shell attachment. Repeated A/B/A Session selection preserves exact
Session and surface identities and keyboard focus. Removing the highlighted
Session exercises Return with remaining results, failed last-good results and
successful empty results; Escape restores the retained terminal. Search text
does not enter either PTY. Draft echo and child reply survive. These are fixture
Sessions and owned local terminals, not configured provider acceptance.

The mounted template proof exercises folded and expanded Task preview, both
return labels while folded, shared Wave/Task disclosure, revision invalidation,
picker/start/restart and captured execution while retaining the terminal draft.
The richer shared fixture separately proves repeated/empty composition and XOR
projection, and that full expansion equals the original graph. These checks do
not establish appearance at either requested capture width.

## Counterexamples and source review

The first navigation run reproduced both reported defects before repair
(`iterate-before.log`). Later test-fixture errors were corrected without changing
the product contract: a Wave selected before its first plan refresh was cleared,
an echo assertion expected text that had not been entered, and synthetic letters
all used the same hardware key code. The final fixture enters its new reply with
Ghostty's text API; palette shortcuts/search/Return/Escape remain dispatched
AppKit events. The native result is recorded in `iterate-session.log` and the
combined pass above.

The initial Rust compile exposed a test expecting the old internal resolver
return type; it now flattens before checking the unchanged XOR routing behavior.
The first mounted Wave-template test found the null default projection described
above (`template-swift.log`); the shared resolver repair precedes the passing
CLI and Swift proofs. No failed run is counted as acceptance.

The simulated code review traced template resolution through capture and UI.
Composition is retained at its existing resolution boundary; no parallel YAML
loader or parent-name grouping exists. Captured ConcreteStep and invocation
storage are unchanged. The old flat-only preview is replaced; the execution
diagram remains because it owns captured execution. Navigation descriptors carry
display/lookup information, never full historical Task snapshots. Existing
generation fencing rejects stale destination reads. No new provider launch,
Started writer, Session inventory, bind writer or terminal pool was added.

The architecture checker still reports the inherited **32/33** SQLite owner map:
`wave_chapters` is missing. All seven other inventories pass
(`iterate-architecture.log`). This remains a failed architecture gate, not a new
permission to change the parent's conversion. User docs were reconciled in
`swift/README.md` and `docs/lf.md`; the generated website copy was refreshed.

## Remaining boundaries

The explicit LOO-298 contract checklist still gates room/bind/attempt work. This
cut does not supply all-kind table-owned Sessions, selected-Run bind fencing,
ordered/current attempts, real-Home conversion or parent landing. No duplicate
storage or decorative room was introduced.
Final source inspection still finds no Bind variant in `SessionCommand`
(`rust/loopflow/src/lf/mod.rs:736`), and `Run` in `session.rs:27` has no
node/iteration/attempt ordinal or authoritative current-attempt projection.

Installed cold/warm Task links, configured providers and retained drafts, live
1440×900/1100×800 captures, Jack's verdict, the one-mount room, bind races and
the complete design's remaining deletion/acceptance obligations stay open.
No publication, Task disposition, installed activation, external message or
Flow navigation decision occurred.
