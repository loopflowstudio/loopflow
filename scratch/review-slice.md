# Slice review — 2026-09-23

Disposition: retain the flow-history and passive-reader foundation; return to
implementation. Capture coverage and continuation are still incomplete, so this
is not a passed slice and does not advance to publication or the human demo gate.
The grouping reduction is coherent and creates no competing authority.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Exact flow history | Repeated skills, retries, Iterate, restart, completion retain attribution atomically without parent wakes | Transactional Task flow facts and explicit Run bindings | Prior eight store tests and controller review; owning transactions and observation exclusion remain unchanged | pass |
| Passive configured read | Read available Task output without a checkout or client takeover | Direct CLI read-only dispatch over existing registry, journals, and recorded native sources | Fresh branch build; four `lf task output LOO-293 --json` pages from `/tmp` | pass |
| Incremental pages | Preserve records and continue without replay | Byte offsets, source identities/revisions, and bounded round-robin transcript reads | Fresh configured pages: 167, 145, 128, 128 records; zero repeated revisions or gaps; same Task and three sources | pass |
| All/new Runs | No seven-day/fifty-Run cap; discover auxiliary Runs | Full manifest scan plus exact durable-ID/issue attribution | Compression proof: 51 historical Runs plus one new Run, two ordered records each; no replay | pass |
| Native live arrival | Prose/tools visible before native Session completion without replacement | Passive Claude/Codex JSONL and OpenCode SQLite readers | Prior configured Codex proof: one prose item, five tool records, same exact client; no fresh all-provider proof | gap |
| Complete capture | All supported native and autonomous/auxiliary output | Summary journals still report limited_capture; Claude/OpenCode live paths remain unproved | Source normalization and current evidence ledger | gap |
| OpenCode revision continuation | Emit only changed revisions at inclusive timestamp boundaries | Timestamp-only updates previously dropped retained hashes and replayed content; fixed here | New regression failed before correction; focused proof recorded below | pass after correction |
| Reset/compaction coverage | Replacement or lost continuation must be explicit | File identity, Session creation, reduced counts, retained-part deletion detected | Prior local reset/compaction tests; arbitrary equal-count older rewrites still unproved | gap |
| Bounded reconnect/history | Bounded discovery and independent history/live cursors that remain usable | All manifests and flow facts reread; one cursor accumulates per-Run/part state and travels in argv | CLI and Swift source; local exec limit reproduction below | gap |
| Shared wire contract | Rust/Swift agree on labels, revisions, availability, gaps | One ordered source list owns records and labels; no attributed-record wrapper | Prior compression Rust and Swift fixture passes; mirrors inspected here | pass |
| Watch experience | Diagram, ancestry, attempts, filters, Follow live, earlier invocations, accessible controls and existing Session links | No TaskWatchSnapshot or Watch UI; Task workspace still offers Changes/Terminal behind runtime/workspace prerequisites | TaskWorkspaceView, CLI symbols, RegistryQuery and Swift model search | gap |
| Complete configured demo | Autonomous/native live output, transition, earlier-stage inspection, auxiliary Run, Iterate return, reopen | CLI reader evidence only; no Watch surface exists | Current configured read is historical continuation, not a live Watch demo | gap |

## Configured proof

Built the branch with `cargo build -p loopflow --bin lf`. Ran the resulting
`lf task output LOO-293 --json` from `/tmp`, with the existing Home selected and
successive next_cursor values passed back. Only counts, event kinds, gaps and
identity comparisons were printed; conversation text and credentials were not.

The four reads returned 167, 145, 128, and 128 records from three sources with
zero repeated `(Run, source, item, revision)` tuples, no gaps, and no stderr.
Observed first-read duration was 2.214 seconds; subsequent reads were 0.322,
0.318, and 0.318 seconds. These are four observations, not a published latency
budget or percentile. All four pages still had history remaining. They prove the
current grouped contract and continuation, not that the reader reached the live
tail or that new output arrived before completion in this review.

No production Task was advanced, provider launched/resumed/attached/interrupted,
or native transcript modified to create evidence. The previous Codex live proof
remains dated implementation evidence; it was not repeated or broadened here.

## Bounded correction

OpenCode parts can receive a new time_updated value without changed JSON content.
The unchanged-hash branch skipped updating the retained timestamp. Advancing the
watermark then pruned that hash, and the next inclusive poll emitted the same
content again. The new production-reader regression failed on that third read.
The correction updates the retained timestamp before skipping unchanged content.
Item identity, revision, provider storage, and the public wire contract stay intact.

## Remaining continuation defect

The public output reader accepts an encoded cursor up to 4 MiB and returns a cursor
whose state grows with observed Runs and retained OpenCode parts. RegistryQuery
passes the whole cursor as one command argument. This host reports SC_ARG_MAX of
1,048,576 bytes; an isolated `/usr/bin/true` invocation accepted a 300 KiB argument
but rejected a 2 MiB argument with E2BIG before starting the process. No large
production Task was manufactured. This demonstrates a transport ceiling below
the accepted input bound, not the exact threshold for every environment.

Resolve cursor transport and bounded state together with independent history/live
continuation. Merely raising a byte limit or reporting unavailable output after
argv overflow cannot satisfy long-history watching. Discovery also still scans
all manifests and flow facts every poll; transcript page bounds do not bound the
whole query. An unrelated corrupt manifest currently fails the whole Task read
rather than returning healthy emptiness; its recovery behavior needs deliberate
handling in bounded discovery.

## Ownership and negative architectural proof

Traced native-source receipt writers at launch/resume and provider callbacks,
read-only registry opening, manifest discovery, source resolution, source readers,
Task binding projection, CLI rendering, RegistryQuery and both wire fixtures.
No watcher launch/control call, credential selection, transcript dual write,
new table/migration, daemon path, direct Swift provider-storage read, or old-shape
compatibility decoder is reachable from the output read. SQLite native reads use
read-only flags; their statements select Session/message/part rows.

The existing launch process writes source provenance beside Session identity.
Watch uses the recorded location or exact recorded account, with explicit gaps
when identity cannot resolve. The Task projection only reads RunBound facts;
it does not choose a next stage or infer completion from output. Flow facts remain
excluded from parent observation delivery. Provider labels still mean the Run's
manifest harness; per-attempt provider changes are not newly represented here.

## Required next implementation

1. Finish capture: configured Claude/OpenCode live proof, complete summary-only
   autonomous/auxiliary tool output, and faithful provider attribution across
   attempts. Explicit source gaps are necessary failure handling, not coverage.
2. Implement bounded discovery and practical cursor transport/state with separate
   history/live continuation. Extend mutable-part tests to edits and new records
   interleaved across page boundaries; complete compaction/reset guarantees.
3. Add shared Watch snapshot/fixtures and the Mac Watch surface at all Task entry
   points; prove stage/Run filtering, Follow live, stale evidence, and completed
   history without a worker or worktree.
4. Run the complete configured demo, then the pinned human gate. Landing and Task
   completion remain owned by the pinned final flow.

Publication rule: the invoked [review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md) says, “When all applicable
`Done when` claims hold and the slice is coherent, publish or refresh the Task PR
with `lf pr publish`.” Capture and continuation gaps above leave that condition
unsatisfied. No publication or landing was attempted.

## Validation in this review

- `cargo build -p loopflow --bin lf`: passed before the configured CLI proof.
- `cargo test -p loopflow --lib opencode_timestamp_only_update --no-fail-fast`:
  the new regression failed before the correction, proving replay on the third read.
- `cargo test -p loopflow --lib opencode_ --no-fail-fast`: 63 passed after the
  correction, including the new regression and existing mutable-part paging test.
  This name filter also includes OpenCode launch, harness, and authentication tests;
  it is not 63 passive-reader tests or configured-provider coverage.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.

Prior store/controller, source-account, and shared fixture passes are explicitly
prior receipts above. No full repository suite or Watch demo ran in this review.

---

# Retained main-view-task evidence (LOO-291)

# Unified navigation slice review — 2026-09-23

Latest: [iteration 10 integrated review](review-slice-iteration10.md). Session-row
return and nested checkout retention now have a passing mounted real-PTY proof.
The fresh configured attempt stopped before interaction at this runner's
Accessibility permission boundary. Publication remains withheld; earlier
configured launch/completion and viewport receipts retain their stated scope.

## Iteration 9 review — viewport proof confirmed; exact focus unavailable

The slice advances the accepted design. Inspected all three configured viewport
captures: bottom rows 168–200 change to rows 1–33, and rows 1–33 remain after
list/details/overview/terminal return. This closes the bounded scroll claim.
The fresh Session-row/nested-layout attempt stopped before any UI action because
AX focus belonged to Warp. No production correction was established by that
observation; publication remains withheld for the remaining configured claims.

Read the Task, current slice and integration amendment. Recovered the complete
Task diff at HEAD `31d34f272` in `/tmp/loo291-review9-diff.json`: `truncated:
false`, 987,846 patch characters, 188 sections. Compared with iteration 8:
153 unchanged, 35 changed/new, none removed; all changed sections were evidence
or guidance. A concurrent native-test addition followed that snapshot, described
below. The later CLI diff hit its size limit; `/tmp/loo291-review9-tracked.patch`
contains the unrestricted tracked diff, with this review's new probe/receipts
under `configured-ui-evidence/iteration9/review/` inspected separately.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Terminal viewport | Navigation preserves a genuinely scrolled terminal | Same history rows return after list/details/overview | Inspected iteration 9 bottom/scrolled/returned screenshots and executed wheel receipt | pass, bounded configured path |
| Task conversation and completion | Fresh scoped launch, retained exact draft, usable original shell after Complete | Existing integrated provider receipt establishes these behaviors | Iteration 8 exact provider text/client and completion receipts; unchanged production | pass at recorded scope |
| Session-row return | Focus the existing attached shell without replacement | Source resolves terminal attachment before selecting its checkout and pane | Fresh configured attempt stopped at exact foreground check | gap, no row interaction reached |
| Nested checkout layouts | Independent inner layouts and processes survive switching/hiding | Existing separate layout owners preserve local state | Prior local proofs; concurrent native test remains under development | gap for configured interaction |
| One authority | Shared reads/identity and window-local surfaces | One Podium caller per inventory read, one root registry; removed types absent | Negative searches, attachment/selection/completion source trace | pass; LOO-284 contract still absent |
| Remaining scope and budgets | Other scopes/destinations, editing, shared actions, external trials and measured budgets | Not established by this viewport trial | Current shared contract and explicit design obligations | gap |

### Fresh configured boundary

Verified the disposable bundle signature and executable hash
`75fdef9e4d35fed6deeb01c3995bddcd5c7e0c6e5b7f88ce733aff01c4ae1abe`.
The fresh single-use probe selected the same installed development CLI/Home and
actual repository, without fixture/capture mode or installed-app replacement.
AX trust was true; AXWindows succeeded with one window. Owned app PID 12244
reported active before and after its activation request, but system-wide AX
reported focused PID 33576, confirmed as `/Applications/Warp.app/Contents/MacOS/stable`.
The console had no locked-screen flag and login was complete. Window raise
returned -25205. These observations do not establish the cause of the activation
disagreement or justify another permission request.

The strengthened precondition stopped before terminal creation, Task selection
or provider launch. No input was sent, and no repeated activation trial followed.
Owned app 12244 exited. Before/after shared reads retain the identical three
Session IDs; LOO-291 remains incomplete at `2026-09-24T04:21:47Z`. This population
differs from the preceding iteration's one-Session receipt and is not a controlled
timing series. [Probe](configured-ui-evidence/iteration9/review/provider-probe.swift),
[log](configured-ui-evidence/iteration9/review/provider.log),
[final receipt](configured-ui-evidence/iteration9/review/receipt.json).

### Source and verification disposition

Traced native wheel input to Ghostty's existing scroll API and retained view;
Session selection to verified shell attachment, checkout selection and native
focus; and completion to shared actions and non-undoable pane reconciliation.
No second viewport, inventory, writer or terminal owner appeared. Negative
searches retain one desktop `query.sessions`/`query.roadmap` caller and one root
registry, with all previously removed navigation/scope types absent. Current
main's SessionRecord still lacks LOO-284 legal actions/display path.

Another writer added `sessionRowRestoresWorktree` during this review. Its source
uses real native surfaces with fixture Sessions and ViewInspector actions. It
is not configured-provider evidence, and no passing result is claimed here.
Preserved it without edits or a competing build. The production completion hash
still matches `adaae303…257fc8d`; the test file now differs from the iteration 8
receipt. That earlier two-test/four-case receipt applies to its recorded source,
not the concurrent extension. No new unit test or aggregate gate ran in this
review. The historical resource-gate failure was not retried.

Next proof requires an owned Loopflow window with **exact system AX focus** before
launch or keyboard input. Use the signed disposable build, allocate fresh
receipts, then exercise the exact newly created Session row and two checkout
groups; verify all original children respond after return. Do not substitute a
user-owned Session or replay a completed identity. Preserve the successful scroll
and provider receipts. Other destinations/scopes and full-Task trials remain
separate obligations. No PR publication, landing, Task completion or PM write
occurred: the skill's “When all applicable `Done when` claims hold” condition is
not met by the outstanding configured row/nested-layout proof.

## Iteration 8 review — integrated provider path passes

The slice advances the accepted design. **Integrated New conversation, exact
draft retention, UI completion and return to the original shell now pass through
the configured provider path.** Publication remains withheld for the narrower
remaining proof gaps below; the locked-desktop observation is superseded for
this trial. No production or test source changed in this review.

Recovered the complete Task diff with `lf task diff LOO-291 --json`:
`/tmp/loo291-review8-diff.json`, `truncated: false`, 822,231 patch characters.
Compared every section with the preceding independent review: 121 unchanged,
37 changed/new, none removed. The executable delta is shell completion and its
rejection-state correction; remaining changes are guidance and evidence. Read
the delta against the accepted design, traced shared reads, scoped launch,
attachment, completion, reconciliation and retained workspace ownership. The
broader target still includes directive editing, shared LOO-284 actions/display
path, conversation design and the human-selected external trials/budgets.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Scoped configured launch | Displayed Task supplies context and checkout without forced destination | UI New conversation publishes an interactive LOO-291 Session in its owned checkout; provider orients to that directive | Fresh AX trial, exact Session and provider history | pass, configured Task/terminal path |
| Local attachment | Embedded provider remains attached to its original shell | Shared terminal ID matches live provider receipt; provider descends from the owned app | Exact Run, PTY attachment and PID/birth receipts | pass; explicit row re-selection not exercised |
| Navigation and draft | A/D and details preserve conversation and unfinished text | Same client survives list/details/overview/return; exact submitted text and reply recorded | Provider history and unchanged client receipt | pass, configured |
| Completion | Resolve Session, retain usable shell and incomplete Task | UI Complete removes shared row, ends provider, shell accepts command; Task shows no Session and stays incomplete | Fresh AX/CLI trial and shell file receipt | pass, configured |
| Rejection | Error survives polling with live terminal and retry | Completion error has independent lifetime in retained SessionItem | Existing before/after native regression; matching source hashes | pass, fixture CLI/real PTY |
| One authority | Shared identity/readings, window-local views | One Podium inventory reader per query and one root registry; no removed navigation types returned | Fresh searches and Rust/Swift comparison | pass, source; LOO-284 remains outstanding |
| Remaining configured behavior | Both destinations/scopes, nested switching and terminal viewport retention | Local proofs remain; this trial covers one Task and one terminal destination | Prior receipts retain their stated limits | gap |
| Timings and full Task | Published budgets, required sample sizes, external work and edit round trip | Two new scoped observations, no qualifying external trial or edit | Monotonic AX/provider observations below | gap |

### Configured receipt

The read-only console check at `2026-09-24T03:41:14Z` reported no locked-screen
flag, login complete and AX trust true. The same runner then launched the
signed disposable `/tmp/loo291-iteration8/Loopflow Proof.app`, SHA-256
`75fdef9e4d35fed6deeb01c3995bddcd5c7e0c6e5b7f88ce733aff01c4ae1abe`.
AXWindows succeeded with one window and the owned app was active. The installed
development CLI/Home stayed explicitly aligned; no fixture or capture mode,
installed-app replacement, permission change or desktop unlock was performed.

Owned app PID 44677 launched Session `run_a02a714cede94a0e84466dbbf175122e`,
provider conversation `01a0d181-4875-70c2-9cea-4c4d6b17543b`, client PID 48366
with birth `2026-09-24T03:41:45.901289Z`. Its shell attachment is
`F431A536-D21D-455A-A74F-0D3586C3FDDF`. The provider made no tool calls.
After navigation its exact final user text was
`Do not use tools or edit files. Reply only LOO291_ITER8_CONFIRMED.` and its
answer was `LOO291_ITER8_CONFIRMED`. Complete resolved only this owned Session;
both owned processes were absent afterward and fresh CLI evidence kept LOO-291
incomplete. The completed identity and single-use probe must not be replayed.

[Executed probe](configured-ui-evidence/iteration8/review/provider-probe.swift),
[interaction receipt](configured-ui-evidence/iteration8/review/provider.log),
[final state and provider messages](configured-ui-evidence/iteration8/review/final-state.json).
Launch to accessible Task was 4,279.2 ms; pressing New conversation to observing
the initial provider response was 10,139.4 ms. These include probe traversal,
polling and setup waits. They are neither pixel-paint measurements nor published
p95 budgets or a controlled before/after series.

### Verification and next slice

Reused the unchanged iteration 8 native proof: two tests/four cases pass for
shell rejection/success and direct completion before/after repository navigation.
Production/test hashes still match `adaae303…257fc8d` and `63042239…286db1`.
No test rerun or broad gate was warranted by a source change. The earlier
resource-preflight failure remains the latest aggregate-gate receipt; it was
not retried or relabeled as current resource-state evidence.

Negative searches still find one desktop `query.sessions` and `query.roadmap`
caller, one root SessionsWorkspaceRegistry, and no SessionScope, SessionContext,
SessionGroup, SessionRowItem, PodiumConsole, PodiumSurface or `_loadHierarchy`.
Current main still lacks projected Session actions/display path. The existing
Complete operation is reused; no new lifecycle matrix or writer was introduced.

Next configured proof should target explicit Session-row return into its existing
shell, two checkout groups with independent inner splits, and a demonstrably
scrolled terminal viewport. Cover the configured external destination and other
visible scopes separately. Preserve this successful provider receipt instead of
repeating it as a substitute. Resolve the verification resource envelope before
the fallback compilation gate. External work remains human-selected; do not count
this Loopflow trial toward it. Under review-slice's “When all applicable `Done
when` claims hold” condition, these remaining gaps preclude publication. No PR
publication, landing, Task completion or PM write occurred.

## Integrated workspace review — 2026-09-23

**Advances the accepted design; publication remains unapproved.** The integrated
navigation, scope-aware entry, nested checkout layouts and appearance correction
are coherent. This review fixed a newly reachable completion gap. Configured
New conversation launch/attachment and external destination handoff still need
proof against this integrated build; earlier provider receipts belong to their
recorded binaries. This review does not complete LOO-291.

**Final concurrent-source check:** another writer extended `shellSessionCompletion`
after this review's final run. Their new inventory-refresh assertion reproduced
the completion error disappearing at the next poll. They preserved the live
Session state and added a per-item `completionError`, cleared on retry; the
existing completion control renders it. This separates action failure from
opening failure without adding a durable authority. Their [before](configured-ui-evidence/integrated-review/concurrent-rejection-before.log)
and [after](configured-ui-evidence/integrated-review/concurrent-rejection-after.log)
receipts show the failure and two tests/four cases passing after correction.
I inspected that bounded change and preserved it without a competing build.
The native receipt and configured shell binary recorded below precede this
concurrent correction; they are not relabeled as testing its source. The newer
receipt supplies focused native proof, not configured-provider or gate evidence.

Recovered the complete Task diff with `lf task diff LOO-291 --json`:
`/tmp/loo291-review-integrated.json`, `truncated: false`, 690,279 patch characters.
Reviewed the integrated source, incoming Rust attachment/current-Wave/forget
changes, Swift workspace and reading owners, fixtures, and prior proof artifacts.
The starting HEAD was b12beba8b plus the supplied integration corrections; those
were checkpointed locally through lf before this review's source edits.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| One Work inventory | Upcoming, autonomous and human Tasks share typed planning rows | Podium owns reads; derived projection preserves unmatched human boundaries | Complete diff, source trace and negative searches; configured Task selection | pass at stated levels |
| Integrated navigation | Actual directive, no-Session state, A/D and retained input | Configured current app preserves exact shell input through details/overview | [Configured shell trial](configured-ui-evidence/integrated-review/shell.log) | pass, live app and registry |
| Shell Session completion | Same shared action remains available after local attachment | Completion now targets the attached Session while retaining its shell; rejection stays visible | [Regression before](configured-ui-evidence/integrated-review/completion-before.log), [after](configured-ui-evidence/integrated-review/completion-after.log) | pass, native fixture with mocked CLI |
| Direct Session completion | Complete cannot create undoable dead panes or disrupt repository return | Existing direct Session behavior survives the correction | Same final focused command, both repository timings | pass, native fixture |
| Scope and destination | New conversation uses displayed subject and configured destination | Task affordance names LOO-291; shared launch path receives scope without TUI/IDE override | Configured affordance, launch source and existing scope tests | gap: actual new provider/app launch not exercised |
| Nested layouts and attachment | Checkout groups retain independent inner layouts; live PTY establishes local attachment | One window pool; outer slots retain inner stores; Rust verifies terminal marker against stdin PTY | Incoming focused receipts, source and shell trial | local proof; configured two-checkout/provider path remains a gap |
| Appearance | Text and controls remain readable in both appearances | Window resolves native scheme and custom palette together | Inspected both integration light/dark live-data captures and source hashes | pass, prior integration rendering binary |
| Verification gate | Both native and fallback configurations compile | Focused SwiftPM proof passes; aggregate runner did not start suites | [Gate](configured-ui-evidence/integrated-review/gate.log), [recovery](configured-ui-evidence/integrated-review/resource-recovery.log) | gap: active main build exceeds resource budget |
| Full Task | Shared LOO-284 contract, directive editing, external trials and published budgets | Still explicitly outstanding | Current main SessionRecord lacks actions/display path; no authorized external workflow selected | gap; not waived by slice proof |

### Bounded correction

New conversation creates a shell pane. Selecting its shared Session correctly
focused that shell, but SessionPaneView's completion action accepted only
`.session` panes. The new native regression reached the real shell surface and
failed because no Complete control existed, for both success and rejection cases.

Completion candidates now include every interactive Session attached to that
shell through the existing local-terminal lookup. Multiple candidates retain
separate titled actions; no arbitrary first record becomes the shell's identity.
All buttons reuse the existing shared completion operation and non-undoable
reconciliation. Only a direct Session surface is released; the shell stays alive.
Rejected completion displays its error and leaves the action available. Completion
busy state resets on success as well as failure, since a shell survives and may
host another conversation. No new lifecycle matrix, DTO, storage or process owner.

Final focused command:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'WorkspaceNavigationProofTests/shellSessionCompletion|WorkspaceNavigationProofTests/workspaceRetainsNativeSplit'
```

Two tests/four cases passed, exit 0. The shell test covers separately completing
two records attached to one terminal, rejected completion, subsequent actions,
retained pane/surface identity and actual child replies. The direct-Session test
covers completion while viewing and after repository navigation, plus existing
Undo, Task, companion, title, draft and viewport assertions. Completion responses
and Session records are fixtures; these do not establish backend provider stop
or promoted-Ask caller release. No Swift edits by this review followed this proof; the concurrent extension is
recorded above.

### Configured result and remaining boundary

Updated only `/tmp/loo291-integration/Loopflow Integration.app` with the current
SwiftPM executable; the installed app was left closed and unchanged. Executable
SHA-256 `99b8b2b9d1964a7c24ea335be5dddc1027ecd938d9b2b7eb1f270a2c250f2d36`.
[Receipt](configured-ui-evidence/integrated-review/receipt.json) preserves source
hashes; [probe](configured-ui-evidence/integrated-review/shell-probe.swift) records
the explicit installed development Home and actual repository.

The exact runner was AX trusted, with one accessible, onscreen, active owned
window. It observed the Task at 3,322.3 ms, inspected its directive and explicit
no-Session state, confirmed New conversation names LOO-291, then used New terminal.
The shell child's output file contains the exact sentence both before and after
Show work list → Work details → All work → Return to terminals. Owned app PID
12966 was terminated and subsequently absent. No provider was launched, moved or
completed, and no user-owned client was touched. This one launch observation is
not a paint/readiness budget, before/after comparison or external-work trial.

The first attempt stopped before terminal creation: SwiftUI inherited the parent
`sessions-surface` identifier on the button. Its visible label was correct. The
[initial receipt](configured-ui-evidence/integrated-review/initial-shell.log) and
probe are retained; the final probe uses the actual label. This was a probe lookup
error, not a new permission or launch failure.

Fresh shared reads at 2026-09-24T02:59:37.187253Z show the canonical repository's
Infrastructure, Intelligence and Product Waves, 19 incomplete Tasks, one scoped
Session, and LOO-291 incomplete. The all-repository roadmap contains 50 Waves;
that count is not this repository's scope. [Population summary](configured-ui-evidence/integrated-review/population.json).

Negative searches find exactly one desktop `query.roadmap` caller and one
`query.sessions` caller, both in PodiumModel, and one root registry construction.
SessionScope, SessionContext, SessionGroup, SessionRowItem, PodiumConsole,
PodiumSurface and `_loadHierarchy` remain absent. Rust owns current-Wave filtering
and attachment; Swift uses no cwd/title attachment inference. Shared Session
actions/display path remain absent from current main; this correction reuses the
existing action, not a replacement authority.

The requested `scripts/test.py --loopflow` stopped at resource preflight, before
any product suite. Supported recovery preserves main's active 19.1 GiB cache
(limit 12 GiB); it remains blocked. No direct invocation bypassed that guard.
The prior Xcode success predates this correction and is not a fresh gate pass.

Next implementation should prove New conversation through its actual configured
terminal and app destinations, including shared Session publication, clicking
the row back into its originating shell, Complete returning to that same shell,
and nested checkout switching. Keep configured terminal viewport retention and
launch/input variability as unresolved observations. Use proof-owned clients;
never substitute a user Session. Resolve the resource envelope before the fallback
build. Do not restart design or repeat unchanged model proofs as substitutes.

Under review-slice's condition, “When all applicable `Done when` claims hold,”
these configured-launch and verification gaps preclude publication. Full LOO-291
still requires LOO-284 integration, directive editing, bounded conversation design,
human-selected external trials and the declared measured budgets/sample sizes.


## Iteration 7 independent verification

See [the integrated review](review-slice-iteration7-independent.md): fresh
configured planning and exact shell-input retention pass; the combined native
completion run fails the visible rejection assertion in one of four cases,
after a concurrent writer's earlier pass. Preserve both receipts. Publication
remains unapproved; the integrated configured provider path and required
paint/readiness measurements still need proof.

## Iteration 6 disposition

The slice advances the accepted design. **Publication remains unapproved:**
exact provider draft fidelity, configured repository/scroll/visual retention,
and controlled before/after timings still lack the required evidence. The former
generic AX blocker is superseded. Real configured planning, native continuation,
UI completion and companion survival now have receipts. Full LOO-291 scope is
unchanged; this review neither lands nor completes the Task.

Reviewed HEAD `62f7266bd` and recovered the complete Task diff:
`/tmp/loo291-review-iteration6.{json,patch}`, `truncated: false`, 6,719 lines.
Compared every section with the prior review: 26 unchanged, 19 changed, none
removed. Changes include the previous review's hidden-Undo fix, configured
receipts, ambient-Wave correction, opening-state reduction and their notes.
Read the changed paths and traced shared reads, identity joins, selection,
opening, native input, completion and retained workspace ownership.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Configured planning | Explicit repository scope; directive/KRs and truthful Session absence | Actual Task details and A/D work with unrelated inherited Wave | Prior navigation receipt plus this review's configured shell trial | pass, configured UI |
| Native continuation and resolution | Exact conversation; explicit transfer; completion preserves Task and companion | Prior owned conversation answered its remembered word; UI completion removed its pane | Archived provider history, retention/completion and final CLI receipts | pass, prior configured binary |
| Exact draft | Preserve every character through navigation | Earlier provider message lost `e `; new shell trial retains exact bytes | Provider-history counterexample and `shell-input.log` | gap for provider; pass for configured shell |
| Current provider launch | New owned conversation becomes an exact selectable Session | Provider replied READY, but current CLI exposed no corresponding Session or Task Run | `provider-launch-gap.json`; stopped only the owned client | gap, cause unestablished |
| Native state retention | Keep split, focus, viewport and repository context | Local native regressions pass; configured shell proves A/D and details/overview retention | Existing native receipts; fresh external AX interaction | pass at stated levels; configured repository/scroll/visual gap |
| Timings | Comparable paint/readiness endpoints on fixed population | Accessible Task observed at 2,889.7 ms in this review; earlier timeout retained | Fresh monotonic AX receipt and earlier scoped series | gap for controlled comparison/budget |
| One authority | Shared inventories and existing workspace/action owners | No removed reader/navigation types returned; opening publishes only per-Session state | Negative searches, shared DTO/source comparison, focused compression receipt | pass; shared integration remains |

### Fresh configured observations

Built current SwiftPM `LoopflowMac` successfully, then refreshed only the
disposable review bundle. Current executable SHA-256:
`959097703d7a0759c9d25959c4aa0b4b46ae527829c1d8269cedf97c6de87e3e`.
Build receipt: `/tmp/loo291-review6-app-build.log`. Earlier provider receipts
remain attributed to their preceding binary; they were not relabeled.

The exact `xcrun swift` probe reports AX trusted, AXWindows success/count one,
one owned onscreen window, and active true. It selected LOO-291 and observed
its authoritative directive and explicit no-Session state, then opened its own
shell. Using the same bulk CGEvent text and 0.2-second pause as the provider
trial, it sent the prefix and suffix first without navigation, then across
Show work list → Work details → All work → Return to terminals. The shell's
`cat` output contains the exact full sentence twice. This is byte-level
configured input/retention proof, not PTY echo or a fixture. No Session was
opened or transferred. Its owned app and child exited afterward.
[Script](configured-ui-evidence/shell-input-probe.swift),
[receipt](configured-ui-evidence/shell-input.log).

This counterexample rules out a consistent truncation of that input event in
the shell path. It does **not** identify why the earlier provider draft lost two
characters. Native key handling forwards printable event text to Ghostty within
the C-string lifetime; inspection established no bounded source fix. Do not
classify the earlier loss as an automation defect without further evidence.

One new disposable conversation was launched with the same explicit Task/TUI
flags as the earlier proof. Provider `01a0d10f-4512-79a3-8fc3-ecaab404d709`
replied `LOO291_REVIEW6_READY` at `2026-09-24T01:37:19.052Z`, with no tool calls.
While it ran, `lf session list --json` returned five records, none for LOO-291;
`lf runs --task LOO-291 --json` returned only the ongoing implementation Run.
The client therefore had no observed exact Session to select. No UI transfer
was attempted. Two Ctrl-C inputs through its own PTY ended it with exit 0;
launcher PID 25047 and provider PID 26842 were absent afterward.
[Bounded receipt](configured-ui-evidence/provider-launch-gap.json).

The installed CLI symlink changed at 18:34 local time and this launch executed
`lf-f1f15f84852857b6c56505aafea27ebe070ab2e11e783fcbdad4867a005ef0c4`.
Its Session JSON now includes `terminal_ids`. This is changed configured-runtime
evidence, not proof that the installation change caused the missing record.
No registry, installed binary, provider history or user client was repaired to
manufacture a selectable Session. Next provider proof must first establish the
owned launch's exact shared Session/Run identity on this configured runtime.

### Source reconciliation and next proof

Main and this checkout still share the earlier SessionRecord hash and lack
LOO-284's legal actions/display path. **lf-new no longer matches**: its read-only
comparison now adds required `terminalIds` / `terminal_ids` (hash
`efefe57f889b7f58f905d879a34e87c834a6a4169be2420336c2a2d640646713`).
Do not infer that its complete workspace design shipped or copy its field alone;
reconcile the complete attachment contract when integrating that work. The live
CLI's extra field does not establish local embedded attachment in this slice.

Negative searches still find one Podium caller per inventory read, one root
workspace registry, and none of SessionScope, SessionContext, SessionGroup,
SessionRowItem, PodiumConsole, PodiumSurface or `_loadHierarchy`. No new writer,
wire contract, action policy, fallback inventory or workspace owner was added.
The opening reduction is coherent: all former optional-return callers discarded
the result, and its request ID never guarded publication or pane selection.
Its existing four-test/five-case receipt passes against unchanged Swift source;
no additional fixture or broad gate was run in this review.

The live machine roadmap at `2026-09-24T01:36:47.976274Z` contains Infrastructure,
Intelligence and Product for the canonical repository (5, 3 and 10 incomplete
Tasks respectively). LOO-291 remains incomplete. This population differs from
the earlier Wave-narrowed observations and cannot supply a controlled timing
comparison. AX traversal counts are not inventory counts.

Next useful proof: resolve or explain the configured owned-launch publication
boundary; verify exact provider text before/after navigation; cover repository
return, both scroll positions and visual quality; then compare defined endpoints
on a fixed population. Keep the earlier failed launch and lost characters as
failures. Optional conversations, directive editing, shared LOO-284/lf-new
integration, human-selected external trials and measured budgets remain the
full Task obligations. No new kickoff or generic permission retry is warranted.

`git diff --check` passes. No production or test source changed in this review.
Under review-slice's condition, "When all applicable `Done when` claims hold",
the remaining configured and timing gaps preclude publication.

## Earlier disposition (before iteration 6)

Advances the accepted design; **not yet approved for publication**. The compact
navigator and shared identity join have focused behavioral evidence. Mounted
native navigation now preserves splits, draft, focus and terminal viewport in
the integration fixture. Navigator scroll also has a native regression covering
repository return. Local completion now also proves that Undo cannot restore the
completed pane while the selected Task and companion remain usable, including
completion after repository navigation. Reconciliation also invalidates Undo
when its hidden Session disappears from shared evidence. Configured
provider interaction/resolution and before/after
timing comparisons remain unproven. No PR
publication, landing, Task completion, PM mutation or external-product trial was
performed by this review.

Scope is the first navigation slice from `main-view-task.md`, not the full
LOO-291 directive. Bounded conversations, Task-directive editing, LOO-284 shared
actions/display path, lf-new's nested workspaces, external trials and measured
budgets remain explicitly outstanding. No new kickoff or alternative design is
needed.

## Iteration 5 review — 2026-09-23

Reviewed HEAD `1aa2e3dca` plus the bounded correction below. Recovered the complete
Task patch (`truncated: false`, 5,660 lines) in
`/tmp/loo291-review-iteration5.{json,patch}`. Compared all sections against the
iteration 4 receipt: 26 unchanged, seven changed by the preceding review fix,
expanded native proof and notes. Read those changes and traced the current
shared-reading, projection, opening, resolution, pane and surface paths.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Completion after navigation | Reconcile the originating workspace without disturbing the active repository | Original pane/surface removed before return; other reading, selection and layout intact | Both serialized `workspaceRetainsNativeSplit` cases | pass, native fixture with mocked CLI |
| Undo respects resolution | Restore hidden views only while their Sessions remain unresolved | Reconciliation now checks the saved Undo layout as well as visible panes | `hiddenSessionReconciliation`, retained/disappeared cases | pass, model regression |
| Navigation continuity | Retain Work, companion, split, draft and viewport through A/D and repository return | Same native surfaces and companion child survive; selected Task stays incomplete | Mounted native proof plus unchanged navigator-scroll receipt | pass, native fixture |
| Shared evidence | Read current scoped planning and exact Session attribution | Product, two Projects, nine incomplete Tasks, three Sessions; zero planned-Task associations | Fresh CLI receipts below | pass, CLI; configured population gap |
| Configured interaction and timing | Operate the real provider path and compare paint/readiness on the same population | Prior AX probe could not reach controls; no timing comparison | Existing configured-probe receipt; fixtures do not substitute | gap |
| Shared authority | One inventory reader and retained workspace owner | Removed paths remain absent; no new Session policy or API | Negative searches and Rust/Swift/main/lf-new comparison | pass, source; LOO-284 integration remains |

The new regression reproduces a distinct Undo failure: Close view hides a Session,
then shared evidence removes it while no visible Session pane remains. The old
early return left its saved layout intact, so Undo restored the absent Session
and took focus from the companion. Reconciliation now also detects stale Session
identities in that saved layout and uses its existing invalidation/notification
path. The retained-Session case proves ordinary Close-view Undo still works.
No new state, lifecycle authority or cleanup owner was introduced.

Before correction, the regression failed four assertions in
`/tmp/loo291-review-hidden-undo-before.log` (exit 1). Final focused command:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'MultiplexerStoreTests/hiddenSessionReconciliation|WorkspaceNavigationProofTests/workspaceRetainsNativeSplit'
```

Two tests, four cases passed; exit 0,
`/tmp/loo291-review-hidden-undo-after.log`. No Swift edits followed; no broad gate
ran. This proves local reconciliation, not backend resolution or caller release.

Live roadmap generation: `2026-09-24T01:03:14.141358Z` (23 September locally).
Receipts: `/tmp/loo291-review-iteration5-{roadmap,sessions}.json`. The Sessions
are two active unbound interactive records and one ready Task FlowStep absent
from the plan. No live Session was opened, moved or resolved. The unchanged AX
boundary was not retried; another launch-only capture would not close its gap.

Negative searches still find one Podium caller per inventory read and one root
workspace registry, with no removed navigation/scope types. SessionRecord in
current main, lf-new and this checkout retains the hash below and lacks shared
legal-action/display-path fields. The slice advances the accepted design; full
LOO-291 scope remains unchanged. Publication remains withheld because configured
interaction and timing claims do not meet review-slice's condition, "When all
applicable `Done when` claims hold".

## Iteration 4 review — 2026-09-23

Reviewed HEAD `442eea4ef` plus the bounded correction below. Recovered the complete
Task patch with `lf task diff LOO-291 --json`: `truncated: false`, 5,455 lines.
Receipts: `/tmp/loo291-review-iteration4.json` and `.patch`. Compared every file
section with iteration 3: 28 are byte-identical; five changed sections contain
the completion test and design, proof, compression and preceding review notes.
Read the delta and traced the current reading, projection, opening, completion,
pane and surface owners. The earlier claim matrix remains applicable with these
updated observations:

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Completion reconciliation | Remove resolved Session without completing Task or ending companion | Reading, pane and surface disappear; selected incomplete Task and original companion remain | Extended `workspaceRetainsNativeSplit`; mocked CLI response, real native surfaces/children | pass, native fixture |
| Completed pane stays removed | Undo restores hidden views, not completed Sessions | Complete now uses existing Session reconciliation rather than undoable pane close | Regression failed before correction and passed after; same companion remains focused and responds | pass, native fixture |
| Navigation continuity | A/D, Work inspection and repository return preserve useful context | Existing surfaces, split, draft and terminal viewport retained before completion | Same focused test; prior separate navigator-scroll receipt | pass, native fixture |
| Current shared evidence | Read real planning and human boundaries with exact identity | Product, two Projects, nine incomplete Tasks and three Sessions; none associated with a planned Task | Fresh read-only CLI receipts below | pass, CLI; configured Task-to-Session population gap |
| Configured interaction | Operate real controls and continue/resolve a provider | Latest normal LaunchServices probe could not reach workspace controls through AX | Implement's `/tmp/loo291-configured-probe.log`; no new external interaction in this review | gap |
| Paint/readiness comparison | Measure before/after on the same population | No measured user-path comparison | Fixture duration and capture delay are not latency measurements | gap |
| Shared authority | One inventory reader and retained workspace owner | Removed paths stay absent; bounded correction reuses MultiplexerStore | Negative searches, Rust/Swift contracts and main/lf-new SessionRecord hashes | pass, source; LOO-284 integration remains |

The prior compression report identified different close/reconcile semantics but
did not prove their consequence. Extending the mounted completion test with Undo
reproduced a concrete failure: the completed Session's pane reappeared, the split
returned, and focus left the companion. Complete called `close`, which stored an
undo snapshot; subsequent reconciliation saw no stale visible pane and returned
without clearing that snapshot. The correction calls the existing
`reconcileSessions` with the remaining Session identities. It removes resolved
panes without creating an undo entry and retains the existing surface release.
Close view and its Undo behavior are unchanged. No new owner, API or lifecycle
policy was introduced. Completion after unmounting remains a separate coverage
gap; this correction does not claim to consolidate every cleanup path.

Focused command:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter WorkspaceNavigationProofTests/workspaceRetainsNativeSplit
```

Before correction: exit 1, four assertions failed in
`/tmp/loo291-review-completion-undo-before.log`. After correction: one test passed,
exit 0, `/tmp/loo291-review-completion-undo-after.log`. This supersedes the prior
mounted-completion receipt for current source. No Swift edits followed; no broad
gate ran. Existing navigator/model/window-isolation receipts retain their stated
proof levels. The completion response is mocked; backend resolution, provider
continuation and blocked-caller release remain unproven.

Fresh `lf roadmap --json` generated at `2026-09-24T00:53:22.142909Z` (23 September
locally); `lf session list --json` exposes two active unbound interactive Sessions
and one ready Task FlowStep whose Work is absent from the plan. Receipts:
`/tmp/loo291-review-iteration4-{roadmap,sessions}.json`. No live Session was opened,
moved or resolved. The existing AX probe was inspected, not repeated: changing
the pane completion method cannot establish accessibility or configured proof.

Negative searches confirm one Podium inventory caller per shared read, one root
workspace registry, and no removed scope/navigation types. Current main, lf-new
and this checkout still share the SessionRecord hash recorded below, with no
shared legal-action/display-path fields. No schema, planning writer, launch path
or fallback inventory changed. The slice advances the accepted design; it does
not complete the broader Task. Next proof remains configured navigation and
provider resolution plus same-population timing comparisons. Publication remains
withheld under review-slice's requirement, "When all applicable `Done when`
claims hold"; those claims still have gaps.

## Iteration 3 review — 2026-09-23

Reviewed HEAD `739e24061`. Recovered the complete Task patch with
`lf task diff LOO-291 --json`: `truncated: false`, 5,271 lines. Receipts:
`/tmp/loo291-review-iteration3.json` and `.patch`. Compared every file section
with the prior review receipt: 25 sections are byte-identical; the eight changed
sections contain the scroll implementation/test, its README and design/proof
notes, plus the preceding review's stronger companion-child assertion. Read that
delta and traced the current navigation, shared-reading and Session-opening paths.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Navigator retention | Preserve list position through A/D, refresh and repository return | Window/repository navigation retains observed offset; remounted SwiftUI view restores it | `navigatorRetainsScroll` checks native clip-view bounds at 1,200 points and zero in another repository | pass, native fixture |
| Native workspace continuity | Retain Session and companion panes, draft, focus and terminal viewport | Existing workspace and surfaces survive navigation; both children respond | Prior strengthened `workspaceRetainsNativeSplit` receipt; unchanged ownership paths | pass, native fixture |
| Current shared evidence | Present scoped planning and exact human boundaries | Live CLI exposes Product, two Projects, nine incomplete Tasks and three Sessions | `/tmp/loo291-review-iteration3-{roadmap,sessions}.json` | pass, read-only CLI; no planned Task has an associated Session in this population |
| Configured interaction | Navigate real controls, continue a provider and reconcile resolution | No new configured interaction result; earlier AX traversal could not reach workspace controls | Prior AX receipt; local regressions use AppKit/ViewInspector | gap |
| Paint/readiness comparison | Compare before/after on the same population | No measured comparison | Test duration and capture delay are not user-path latency | gap |
| Shared authority | One planning/Session reader and retained workspace owner | Removed paths remain absent; no launch or wire-contract change | Negative searches, Rust/Swift contract inspection and main/lf-new SessionRecord hashes | pass, source; LOO-284 integration remains |

The new regression checks actual viewport restoration, not merely the saved
CGFloat. It reproduces Podium's repository identity boundary by remounting
SessionsView with `.id(repoPath)`. Geometry writes capture the navigation owner
for that render; terminal layout and Session reconciliation remain separate.
No bounded source correction was established. The implementation advances the
accepted first slice without creating a competing workspace or Session authority.

Reused the existing final focused receipt:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter WorkspaceNavigationProofTests/navigatorRetainsScroll
```

One test passed, exit 0; `/tmp/loo291-navigator-scroll-final.log`. Production and
test content have not changed since that pass. No tests were rerun in this
review; no broad gate ran. Earlier native/model receipts retain their stated
proof levels. `git diff --check` passes.

The live roadmap was generated at `2026-09-24T00:42:43.244651Z` (23 September
locally). Two Sessions are active unbound interactive records; the third is a
ready Task FlowStep whose Work is absent from this plan. This supersedes the
prior two-record count without establishing why membership changed. No Session
was opened, moved or resolved. The previous inaccessible AX boundary was not
retested; another static capture would not close the interaction gap.

Current main, lf-new and this checkout still have the same SessionRecord hash
recorded below. The shared legal-action/display-path fields remain absent.
Searches confirm one Podium caller per inventory read, one root workspace
registry, and none of the removed scope/navigation types. No planning writer,
schema, lifecycle policy or launch authority was added.

Next proof remains the configured interaction trial below and same-population
timing comparison. Navigator retention now has local evidence and should not be
listed as wholly untested. Full LOO-291 requirements remain unchanged. Under
review-slice's condition, "When all applicable `Done when` claims hold," this
review cannot publish: configured interaction and timing claims still have gaps.

## Iteration 2 review — 2026-09-23

Reviewed HEAD `7105903ec` plus the bounded test correction below. Recovered the
complete Task patch with `lf task diff LOO-291 --json`: `truncated: false`,
5,020 lines; `/tmp/loo291-review-iteration2.json` and the extracted `.patch`.
Compared the full change set with the previous reviewed state and read the
intervening delta: one mounted native test and notes, no production changes.
Rechecked the reachable navigator, detail, reading, Session-action and native
ownership paths against the accepted design and its forbidden outcomes.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Native A/D and detail navigation | Preserve surfaces, split, focus, draft and scroll | Same Session and companion PTYs survive toolbar navigation and repository return | `workspaceRetainsNativeSplit`, final focused command below | pass, native integration fixture |
| Retained children remain interactive | Input reaches both original children after navigation | Both cat replies are required in addition to PTY input echo | Strengthened final assertions | pass, local children; provider continuation remains a gap |
| Current planning and Session evidence | Read shared planning and exact Session identity | Live CLI reads expose Product, two Projects, nine incomplete Tasks and two unbound interactive Sessions | `/tmp/loo291-review-iteration2-{roadmap,sessions}.json` | pass, read-only CLI; no live Task-to-Session association in this population |
| Complete configured interaction | Real controls, provider continuation, resolution and pane reconciliation | No new configured interaction result; prior AX probe could not reach workspace controls | Earlier AX receipt below; fixture actions use ViewInspector | gap |
| Navigator scroll and timings | Preserve list position and compare paint/readiness on the same population | Terminal viewport is covered; navigator scroll and timing comparison are not | Source and fixture coverage inspection | gap |
| One shared authority | One planning/Session reader and retained workspace owner | Removed paths remain absent; LOO-284 fields remain unavailable | Negative searches and current Rust/Swift/main/lf-new comparison | pass, source; full contract integration remains Task scope |

The native test originally accepted any occurrence of `companion-alive`. PTY
input echo alone could satisfy that assertion without proving the companion
child responded. Replaced it with the same two-occurrence requirement used for
the Session draft, and waited for both responses within the existing deadline.
This is a stronger behavioral assertion, not a production correction or a new
launch path. Both children and surfaces remain test-owned.

Final focused command:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter WorkspaceNavigationProofTests/workspaceRetainsNativeSplit
```

One test passed, exit 0; `/tmp/loo291-review-mounted-child-proof.log`. No Swift
edits followed. No affected-suite or full gate ran. Existing model/selection,
manual-focus and window-isolation receipts remain evidence at their stated
levels; they were not rerun. `git diff --check` passes.

Live roadmap generation: `2026-09-24T00:31:44.394125Z` (23 September locally).
The current two-Session observation supersedes the earlier three-record count;
it does not establish why the third record disappeared. No live Session was
opened, moved, resolved or otherwise mutated. The prior AX failure is historical
evidence, not a fresh permission diagnosis. Repeating a launch-only capture would
not prove the missing interaction, so this review used the corrected native
fixture as the closest local behavioral path.

Rust/Swift Session and roadmap mirrors retain the same fields. Main and lf-new
SessionRecord files still match the hash below; shared legal actions/display
path are absent. Searches find one Podium caller per inventory read, one root
workspace registry, and none of the removed scope/navigation types. No second
writer, fallback inventory, lifecycle policy or workspace owner was introduced.

Next work is the configured interaction trial below, including navigator scroll
and same-population timing observations. Optional conversations, required lf-new
workspace integration, directive editing, LOO-284 integration, external proving
work and measured budgets remain the full Task target. The new fixture advances
the first slice without changing that target. Publication remains withheld under
review-slice's requirement that all applicable Done When claims hold.

## Earlier review after native restoration — 2026-09-23

Reviewed HEAD `39d4c4653` using the complete Task diff (`truncated: false`,
4,736 lines), the changes since the previous review, and the reachable owners.
Receipt: `/tmp/loo291-review-restored.json`; extracted patch:
`/tmp/loo291-review-restored.patch`. The native changes advance the accepted
design: one retained view owns the focus request, and the CoreVideo adaptation
uses Ghostty's existing renderer. No further bounded source correction was
established in this review.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Real planning paints | Compact Wave/Project grouping with all incomplete Tasks | Product, Desktop and Company Dogfood render nine Tasks from the actual registry | Temporary bundle, live capture; `review-live-planning.png` | pass, real-data rendering only |
| Failed reads stay explicit | Missing evidence cannot look like healthy emptiness | Bare executable shows Unknown, Sessions unavailable and Planning unavailable when its bundled helper is absent | `/tmp/loo291-review-live.png`, inspected image | pass, real failure rendering |
| Native draft/focus retention | Preserve input through hiding and resizing | View-owned request also preserves manually acquired Task-terminal focus | Existing final compression receipt, one real-PTY test | pass, fixture; no new test run |
| Complete navigation interaction | A/D, exact Session access, split/draft/scroll retention and resolution | Source and focused fixtures support it; current external automation did not reach workspace controls | AX/capture attempts below | gap; publication remains unapproved |

Fresh read-only `lf roadmap --json` and `lf session list --json` succeeded.
The roadmap receipt is generated at `2026-09-24T00:13:52.840193Z` (23 September
locally). Sessions now contains **three** records: two active unbound interactive
Sessions and one ready Task FlowStep whose Work is absent from the Product plan.
The earlier two-Session observation below is historical. No Session was opened,
moved, resolved or modified by this review.

Attempted the actual built app before falling back to the existing live capture
mode. `AXIsProcessTrusted()` and `CGPreflightScreenCaptureAccess()` return true
for the probe process. Nevertheless, AX traversal of this review's app returns
application/menu elements even through `AXWindows`, without workspace controls;
setting `AXManualAccessibility` returns `-25205`. External capture of its own
window reports `could not create image from window`. These observations do not
establish the cause or prove that the hosted UI runner has permission. Receipt:
`/tmp/loo291-review-interaction-ax.log`. No blind clicks or global keystrokes were
used to work around the inaccessible controls.

The app's existing `live` capture mode works. The initial bare SwiftPM launch
correctly reports the missing bundled `lf`; it is not a configured-app verdict.
A temporary `/tmp/loo291-review/Loopflow Review.app` then used this checkout's
unchanged SwiftPM executable/resources and the existing development control
configuration pointing to the same installed `lf` used by the live CLI reads.
Its distinct bundle identifier avoids replacing the installed app. With
`LOOPFLOW_UI_TEST_MODE=live`, a snapshot path, a 12-second capture delay, and
`--repo /Users/jack/src/loopflow.main-view-task`, it renders the real scoped plan,
three-Session count, and planning-read timestamp. No fixture records are injected.
The capture delay is not a measured paint time or budget. This proves static
rendering, not interaction, native terminal rendering or external-product use.
All review-owned app processes exited; no user app was stopped.

Rechecked Rust/Swift Session and roadmap fields and current main's Session DTO;
the three Session mirrors retain the hash recorded below. Main and lf-new still
have no shared legal-action/display-path contract to consume. Negative searches
still find one Podium inventory caller per shared read, one root workspace
registry, and none of the removed scope/navigation types. No new persistence,
writer, compatibility path or launch authority was introduced.

No production or test source changed, so no tests were rerun. The final manual
focus correction's receipt is `/tmp/loo291-compress-manual-focus-after.log`;
the previous 25 model tests and independent native window-isolation receipt
remain applicable at their stated proof levels. `git diff --check` passes.

Next work remains the configured interaction trial described below, including
retained split/scroll state and a disposable authorized human boundary. Restore
access to actual workspace controls on that host before attempting it; another
launch screenshot does not close that gap. Full LOO-291 scope is unchanged.

## Evidence matrix

Pass describes the stated proof level; model fixtures do not establish native
interaction.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Unified inventory | Incomplete autonomous, upcoming and human Tasks share compact Wave/Project groups | Derived planning rows, stable planning IDs, ranked Tasks, no Session prerequisite | WorkspaceNavigationTests.workDoesNotRequireSessions; navigator source | pass, model; configured appearance gap |
| Exact Session association | Typed durable Work edge; never name/cwd/planning-ID guesses | Joins runtime.workId by kind; retains multiple and unmatched Sessions | everyHumanBoundaryRemainsReachable; current Rust/Swift SessionRecord comparison | pass, fixtures |
| Completed and top-level work | Completed-with-Session, repo/Wave/Project and missing planning remain reachable | Completed rows retained when associated; unmatched records remain in Other open Sessions | everyHumanBoundaryRemainsReachable; live read has two unbound Sessions | pass, model; live continuation gap |
| Details and no-Session | Directive, condition/reason, Project definition/KRs, Activity, PR/worktree and explicit absence | Existing inspectors/actions reused; zero only after a readable association | inspectorShowsPlanning, unavailableAssociationIsNotNoSessions; WorkSurfaceView/subjectSessions source | pass, model/view |
| A/D presentation | Toggle list without recreating workspace or launching provider | Same retained workspace and mounted multiplexer; presentation state separate | navigationRetainsWorkspace; workspaceRetainsNativeSplit | pass, model/native fixture; configured interaction gap |
| Repository and search retention | Restore selection/expansion/search/splits/focus across navigation | Existing registry and per-repository navigation, expansion preserved during search | navigationRetainsWorkspace; unavailable/truncated regression; mounted native repository return | pass, model/native fixture; configured search and navigator scroll gap |
| Unavailable evidence | Failure is never healthy empty; keep useful context | Last-good readings/errors; unknown badge; partial planning warning; selected Work retained | unavailableIsNotEmpty, lastGoodSessionsSurviveRepositorySwitch, new regression | pass, model/view |
| Responsive Session access | Planning cannot gate Session publication/opening | Session read publishes independently; pane selection precedes asynchronous preparation | sessionsArriveBeforePlanning; openSession source | pass, model; measured latency gap |
| Human lifecycle | Exact open/Move here; resolution does not complete Task | Existing shared operations; callback removes resolved Session from reading | Existing SessionsStore focused receipt; resolutionKeepsTask | pass, fixtures; configured resolution gap |
| Native lifetime and input | Same surfaces/processes/drafts/splits; hidden terminals relinquish input | Retained pool; native view consumes focus requests on attachment/transition; unavailable CoreVideo uses timer rendering | hiddenTerminalPreservesDraft, releaseSurfaceIsWindowLocal and strengthened workspaceRetainsNativeSplit | pass, real PTY fixtures; configured provider interaction gap |
| One authority | Remove root switch/cascade, duplicate Session polling and labels lookup | Podium reads; projection derives; one window registry retains surfaces | Negative searches and complete Swift diff review below | pass, source |
| Source freshness | Distinguish read timestamp from provider sync freshness | Toolbar labels snapshot generation and explains unavailable sync timestamp | SessionsView toolbar, README | pass, honest limitation; full freshness integration remains |
| External trials/budgets | Human-selected workflow, authorized edit, measured long-lived-registry trials | Not implemented/proven in this slice | No workflow selected; no timings collected | gap, remaining Task scope |

## Bounded corrections made

The existing per-repository selection was cleared on return after a successful
roadmap response containing unavailable planning. `setRepoPath` called
`clearSelectionIfOutsideScope`, which treated an unresolvable saved Task as
absent. The new test reproduced `model.selection == nil` after switching away
and back. Switching now restores its existing navigation state without that
redundant clearing. Complete planning refreshes still reconcile removed Work.

The refresh check also accepted truncated planning as complete. It now requires
available, untruncated Projects and no unavailable Project entries before
clearing a missing selection. The same regression covers unavailable and
truncated responses, retaining the selected Task and its unmatched Session.
This changes no lifecycle or ownership boundary.

## Commands and observations

- Read the complete Task patch through `lf task diff LOO-291 --json` (reported
  `truncated: false`), including committed implementation and compression edits.
  Local receipt: `/tmp/main-view-task-review-diff.json`; extracted Swift diff:
  `/tmp/main-view-task-review-code.diff`.
- Live read-only `lf roadmap --json` and `lf session list --json` succeeded.
  Roadmap generated at `2026-09-23T23:42:34.733612Z` contains Product, two
  Projects and nine incomplete Tasks. Sessions contains two unbound records and
  no Work-bound Session. These are configured CLI observations, not proof of a
  live Task-to-Session UI trial. Raw observations remain in
  `/tmp/main-view-task-review-{roadmap,sessions}.json`; no identities or outcomes
  were fabricated to fill the missing population.
- Pre-fix regression: one test failed at the saved-selection assertion.
  `/tmp/main-view-task-review-regression.log`.
- After repository restoration fix:
  `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
  'WorkspaceNavigationTests|PodiumModelTests'` passed 25 tests, exit 0.
  `/tmp/main-view-task-review-proof.log`.
- Final partial-plan correction:
  `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
  WorkspaceNavigationTests/unavailablePlanningPreservesRepositorySelection`
  passed one parameterized test with both unavailable/truncated cases, exit 0.
  `/tmp/main-view-task-review-partial-proof.log`.
- Reused the prior 35-test compression receipt for unchanged Session actions;
  did not run the broad gate. Final source includes the subsequent two bounded
  selection corrections with the focused receipts above.
- Re-read native failure receipts: the new draft test has a nil terminal surface;
  unchanged GhosttyTerminalInputTests.releaseSurfaceIsWindowLocal likewise fails
  before behavior assertions. Logs: `/tmp/loo291-native-focus-proof.log` and
  `/tmp/loo291-existing-native-proof.log`. The subsequent diagnostic in
  `native-surface-diagnostic.md` supersedes the hypothesis that this process has
  no rendering environment: it sees a screen and Metal device. Ghostty reports
  `error.OutOfMemory`, also reproduced by a minimal AppKit host without user
  configuration or navigation code. The allocation failure's cause remains
  unknown. Hosted UI test sources were inspected but not executed.

## Negative architectural proof

Searches under the reachable Mac root and tests find no SessionScope,
SessionContext, SessionGroup, SessionRowItem, PodiumConsole, PodiumSurface or
_loadHierarchy. `query.sessions` and `query.roadmap` in the desktop path each
have one owner, PodiumModel. SessionsStore performs existing Session actions
and maintains local prepared/opening/error state; it no longer polls or resolves
labels. The root constructs one window-local SessionsWorkspaceRegistry.
WorkspaceProjection has no persistence/writer/launch operations. No schema,
backend DTO, migration, compatibility adapter, planning write or new Session
legality matrix was added. Existing Task controls remain in inspectors; removed
console fleet switches were not restored.

The SessionRecord files in this checkout, canonical main and lf-new are
byte-identical (SHA-256
`e020dd7922d735ce2406dbbd38485b372ec8340b57891ec6bc7da05fb005485d`).
The Rust mirror still has the same fields, with no projected legal actions or
display path. This slice preserves the existing native action path; consuming
LOO-284 remains a real integration requirement. Sibling checkouts were read
only. The existing repository workspace is retained, not advertised as lf-new's
unimplemented per-worktree layout.

## Next proof, without redesign

Ghostty surface initialization is restored; the previously failing native
draft/focus and window-isolation tests now pass. See `native-surface-diagnostic.md`
for the CoreVideo failure, timer-rendering adaptation, and focus correction.
These focused fixtures do not waive configured proof or establish visual quality.

Use the configured native app on a rendering-capable, permissioned host. In one
repository show autonomous/upcoming/human Tasks; open the exact existing Session;
retain an unfinished draft in a split beside a running shell. Inspect Task and
Project details, toggle A/D, search and collapse groups, switch repositories and
return. Verify the same surface/process identities, draft, layout, focus and
scroll positions. Confirm search input remains in the field during polling and
resize, hidden terminals receive no input, and explicit Continue restores focus.
Complete a disposable authorized Session and prove pane reconciliation without
Task completion. Do not transfer or resolve the human's current Sessions merely
to manufacture evidence. Retain the native regression alongside that configured
trial. Until this succeeds, the skill's publication condition is unmet.

## Second review after native diagnosis — 2026-09-23

Disposition remains **not approved for publication**. Re-read the complete
current Task patch via `lf task diff LOO-291 --json` (`truncated: false`), comparing
its source delta with the already reviewed patch. Receipts:
`/tmp/main-view-task-review-current.json` and
`/tmp/main-view-task-review-delta.diff`. The intervening implement/compress passes
added diagnosis and review notes, without production changes. The claim matrix
above still applies, with this additional corrected selection path:

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Portfolio refresh preserves Work context | Repository discovery cannot declare Work removed during partial planning | Removed discovery's redundant selection reconciliation; complete planning refresh still reconciles | Extended unavailablePlanningPreservesRepositorySelection, both cases; PodiumModelTests | pass, model |

The regression first failed for both unavailable and truncated planning after
`refreshPortfolio(initialRepoPath: nil)` cleared the selected Task. The final fix
removes that one reconciliation call. Explicit selection validation and complete
planning refresh retain their existing behavior. Moving the completeness check
into every selection call was rejected after the existing missing-selection test
caught its changed behavior; that approach is absent from the final source.

Final focused command passed 25 tests, including both partial-planning cases:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter 'WorkspaceNavigationTests|PodiumModelTests'
```

Exit 0; `/tmp/main-view-task-review-portfolio-final.log`. Pre-fix failure:
`/tmp/main-view-task-review-portfolio-before.log`. Intermediate rejected fix:
`/tmp/main-view-task-review-portfolio-after.log`. No production/test source edits
followed the final pass. This is local model/view evidence; no native or external
trial is included in the count.

Repeated negative searches confirm no removed navigation/scope types returned,
one Podium caller for each shared Session/roadmap inventory read, and one root
window registry. No launch path, DTO, schema, migration, Session action policy,
or native ownership changed. The slice advances the accepted design, with the
same native proof boundary and remaining LOO-291 scope. Nothing was published,
landed, or marked complete.
