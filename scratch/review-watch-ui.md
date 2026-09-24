# Inline Watch review — 2026-09-24

Disposition: the inline inspection slice at `01148246b` plus its working tests
and README passes at the local proof level. The prior review's navigation and
rendering gaps are closed. Refresh the existing in-progress PR; this is not
approval to land or complete LOO-293.

Read the accepted Task directive, current slice and complete target in
`restore-task-watching-with-live.md`, the prior foundation reviews, and the
complete delta from their reviewed inspector at `b0aa670e6`. Captured the full
base-to-working-tree binary patch in `/tmp/loo293-inline-review-complete.patch`;
the inherited workspace and foundation evidence retains its earlier review
scope. This review introduces no production or test changes.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Inline access | Primary Watch action changes selected Task content | Work details and toolbar select the same conditional Watch view; legacy sheets reuse it | Source trace and `watchContentAndRetention` | pass, local |
| Completed history | Completed Tasks need no open Session | One planning projection retains them; toggle, search, selection and Session presence determine visibility | `completedHistoryIsReachable` | pass, local; limited to Tasks in the roadmap |
| Retained inspection | Independent Task/repository selection survives navigation | Existing navigation owns per-Task stores; terminal registry/layout remains separate | `watchContentAndRetention` | pass, local |
| Hidden reads | Hidden Watch cannot keep reading | Conditional mount, Task/repository identity and existing query cancellation | Hidden-view assertions, `queryCancellation`, `reopeningDuringRead`; source trace | pass, local |
| Historical/stale facts | Refresh preserves selection and last-good evidence | Exact invocation/stage selection and request identity | `selectionAndStaleEvidence` | pass, local |
| Native layout and build | Watch fits the unified workspace | Native stage list, invocation picker, attempt cards and Task header render together | Mac linked in focused build; inspected `/tmp/loo293-watch-inline.workspace.png` | pass, rendered fixture |
| Configured passive read | Read a Task without its worker/worktree and report missing facts | Exact Task, three attributed Runs, no retained invocations, explicit unavailable position | Fresh CLI read from `/tmp`, receipt below | pass, configured historical read |
| Complete Watch | Live labeled feed, stage/Run filters, Follow live, paging, native coverage, Session links and demo | Still incomplete | Current design and reachable source | gap, required in this Task/PR |

The existing final command `swift test --package-path swift --filter
'WorkspaceNavigationTests|TaskWatchTests'` passed 18 tests and linked the Mac
product (`/tmp/loo293-unified-watch-tests.log`). The subsequent focused
`TaskWatchTests/renderSnapshot` command passed both standalone and integrated
render cases (`/tmp/loo293-watch-inline-render.log`). No executable changes
followed those receipts. They were inspected, not rerun for this review. The
integrated image shows readable navigation, stage selection and attempts without
clipping. It is a fixture, not configured live output or keyboard-interaction
proof. The retained-layout assertion is model evidence, not a new provider trial.

A fresh `target/debug/lf task watch LOO-293 --json`, run from `/tmp` with the
published Home/database selected explicitly, returned
`task_97e04da37e5e4231bde30f4d1d93a5fa`, three Runs, zero invocations and
`position_unavailable` in 0.346 seconds, with empty stderr. Receipt:
`/tmp/loo293-inline-review-watch-receipt.json`; payload:
`/tmp/loo293-inline-review-watch.json`. Binary SHA-256:
`e22737932729cf60494ffc430003fb4e02ba976e599cf6cddd562e6d97456f79`.
This is the existing branch reader binary, not a fresh current-head build.
The command opens the registry read-only; no provider or production history was
changed. This population has no retained plan with which to demonstrate live
stage progress. The exact runner reports AX trust true; that is not an app
interaction verdict or a reason to recycle earlier permission failures.

The additional Xcode compile check (`uv run python scripts/test.py --loopflow`)
stopped at resource preflight before any product suite ran: main's active build
root uses 19.1 GiB against its 12.0 GiB budget. Supported
`scripts/resource_envelope.py --recover` also returned failure and preserved that
active root. Logs: `/tmp/loo293-inline-review-xcode.log` and
`/tmp/loo293-inline-review-resource-recovery.log`. No Xcode compile pass is claimed;
this remains a validation gap on the in-progress PR, separate from the passing
SwiftPM Mac build. The resource boundary was not bypassed.

Negative architectural review: Watch calls only RegistryQuery.taskWatch; its
store/view neither parses provider files nor controls Sessions. The primary
Work Watch sheet and its terminal-store observation are gone. Roadmap and Wave
detail retain their existing workspace destinations. Podium remains the sole
desktop Sessions/roadmap inventory reader, and Watch introduces no second
planning inventory, flow reducer, persisted transcript, lifecycle writer or
provider-launch path. Human checkpoint links remain absent; Flow Session IDs
must not be inferred from provider Run IDs. Manual refresh remains appropriate
until discovery and retained cursor state are bounded.

No bounded source defect was found. The slice advances the accepted design.
Next implementation must finish bounded discovery/state and independent
history/live continuation, then connect the feed, filters/Follow live and exact
human Session links. Preserve all-provider passive capture and the full
configured/human demo gate; publication of this slice does not waive them.

---

# Earlier Watch inspection review — 2026-09-23

Subsequent implementation proof: the final inline navigation command passed 18
tests in `/tmp/loo293-unified-watch-tests.log`. The window-backed rendering test
now also mounts the unified workspace with its Work list visible; both render
cases pass in `/tmp/loo293-watch-inline-render.log`. Visual inspection of
`/tmp/loo293-watch-inline.workspace.png` confirms the Task header, navigation,
native stage selection and attempt cards fit together. README describes inline
Work navigation and completed-Task access. These close the bounded local proof
gaps identified below; they do not supply live output or the configured demo.
The historical review and publication boundary below retain their original scope.

The manual snapshot inspector at `b0aa670e6` passes this review at the local
proof level. During review another writer advanced HEAD to `01148246b`, changed
the active slice to inline workspace navigation, and began editing its tests.
That material change invalidates a combined-head publication judgment. This
review does not publish, land, or settle the Task.

## Scope and evidence

Read the accepted LOO-293 directive, current design, prior reader/snapshot and
compression reviews, and the Watch implementation. Compared the integrated
entry points and shared query path with checkpoint `021fc6470`; inspected the
subsequent inline-navigation commit and its working test delta. Imported
LOO-291 receipts retain their own scope and do not prove this Task's Watch demo.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Exact stage inspection | Repeated names retain distinct coordinates and attempts | Invocation ID plus step index; separate attempts and exact transition targets | Existing post-rebase five-test pass; source and fixture inspection | pass, local |
| Historical and stale evidence | Preserve selection through transition/completion and failed reads | Retained snapshot/error and request identity; obsolete responses rejected | `selectionAndStaleEvidence`, `reopeningDuringRead` in the same receipt | pass, local |
| Worker-independent access | Watch works with no runtime or surviving worktree | Watch bypasses Changes/Terminal prerequisites; query uses nil cwd | `workspaceWithoutRuntime`; configured CLI from `/tmp` | pass at these boundaries |
| Passive observation | Never attach, resume, replace, approve, or interrupt a provider | Typed `task watch` read; cancellation owns only the query child | Source trace; before/after-spawn cancellation test | pass, local |
| Visible plan | Show stages, ancestry, separate attempts, provider labels and honest gaps | Native List/Picker, attempt cards and coordinate links | Inspected `/tmp/loo293-watch-snapshot.png` | pass, rendered fixture; not a live demo |
| Existing entry points | Work, Roadmap and Wave detail open Watch | Shared inspector was reachable through all three at `b0aa670e6` | Source trace through unified SessionsView details and workspace sheets | pass, source; Work route subsequently changed |
| Inline navigation | Retain independent Task inspections, hide/cancel reader, preserve terminals | New `.watch` content and navigation-owned stores; conditional view mount | `01148246b` source; two working navigation tests have no reviewed final receipt yet | gap |
| Completed Task history | Reach completed Tasks without an open Session | Projection retains them; navigator toggle/search/selection governs visibility | New source and working `completedHistoryIsReachable` test | gap pending focused proof |
| Complete Watch | Live feed, filtering, Follow live, all capture, human Session links and configured demo | Still incomplete | Current design and reachable source | gap, remaining Task scope |

The reusable post-rebase receipt is `/tmp/loo293-rebase-watch-tests.log`:
`swift test --package-path swift --filter TaskWatchTests` passed all five tests
and linked the Mac product. No Watch model/view/store/test changes intervened
between that pass and the inspected checkpoint. No identical tests were rerun
merely because review began. The image predates the inline-navigation change;
it proves the inspector's appearance only. Hosted Xcode compilation and
keyboard interaction are not established by these receipts.

A fresh configured read used the existing branch CLI from `/tmp`, with the
published Home selected explicitly and only the read-only Watch operation:
`lf task watch LOO-293 --json`. It returned the exact Task
`task_97e04da37e5e4231bde30f4d1d93a5fa`, three Runs, zero retained invocations,
and `position_unavailable` in 0.382 seconds, with no CLI stderr. Receipt:
`/tmp/loo293-watch-review-configured.json`; executable SHA-256
`e22737932729cf60494ffc430003fb4e02ba976e599cf6cddd562e6d97456f79`.
This existing executable was not rebuilt for the new UI commit. The observation
proves configured historical reading and truthful missing evidence, not current
head build fidelity, live output arrival, or configured UI interaction. No
provider or production Task/transcript was mutated.

## Architecture and next proof

Watch reaches RegistryQuery's typed read through the existing cancellable query
runner. The inspected Watch path has no Swift file/database reader, YAML plan
reconstruction, skill-name join, provider control, polling timer, or durable
transcript/lifecycle store. Attempts are labeled recorded states, not inferred
process liveness. The new per-repository navigation dictionary retains UI state;
it does not author history. Existing terminal ownership remains separate.

No bounded production defect was established in the reviewed inspector. The
concurrent inline implementation advances the design, but must finish its own
proof before publication:

1. Run the final focused navigation/Watch command against stable source. Prove
   completed-without-Session access through the real navigator action, distinct
   Task and repository selections, and absence of the Watch view under details,
   terminals and overview. Preserve the existing stale/cancellation cases.
2. Inspect the integrated inline layout and confirm its selection header and
   return controls; the earlier isolated inspector image cannot prove placement.
   Keep fixture evidence separate from the full configured demo.
3. Reconcile README and slice notes with inline Work navigation while retaining
   the existing Roadmap/Wave sheet destinations. Review the final delta, then
   refresh the in-progress PR if this slice's applicable claims hold.

The invoked review-slice instruction says: “If a material change invalidates
prior review, return precise direction for the next slice rather than approving
stale evidence.” That applies to the changed navigation and active test edits.
The live feed, bounded discovery/state, independent history/live continuation,
complete native/autonomous capture and configured human demo remain in this
same Task and PR. This report does not reduce those acceptance requirements.
