# Navigation slice review

2026-09-26 · LOO-303 · review-slice

Reviewed the complete active-PR diff from `c832aaede` through `abfa364b0`,
the supplied compression changes checkpointed as `5def26ba0`, and the repairs
below. The [approved design](workspace-ux-on-data-model.md), Jack's evening
scope correction, and [interactive review](workspace-ux-review-feedback.md)
govern. The [implementation proof](workspace-navigation-proof.md) records
earlier evidence; this note does not turn those receipts into installed acceptance.

The navigation slice advances the approved architecture. It needs the remaining
navigation acceptance below before publication. Folded templates are the next
independent implementation cut; room/bind and attempt presentation still require
the parent contracts. No Flow navigation or Task disposition is selected here.

## Findings and repairs

1. **Repository-qualified lookup escaped its scope without Git metadata.**
   A new real CLI fixture registers two repositories with different Tasks sharing
   `PRD-52`; the second Task is completed. Global lookup correctly returned both,
   but lookup from the second registered directory also returned both. The existing
   generic roadmap scope deliberately becomes global outside Git. Exact Task
   destinations now use `CanonicalRepo::discover(cwd)` when `--all` is absent,
   retaining linked-checkout canonicalization and directory-based cached reads.
   Ordinary roadmap behavior is unchanged. The failing observation is preserved
   in `review-scope-before.log`; both exact-lookup tests pass after the repair.
2. **A historical Task offered a palette action with no visible destination.**
   `WorkSurfaceView` suppresses the Flow panel when planning is unavailable and
   no capture exists. The palette still offered Choose Flow, whose picker was
   therefore invisible. Palette availability now follows that presentation rule;
   Monitor remains available. Rename also follows the existing breadcrumb rule
   when the canonical title is unavailable on this Home. Activation still
   rechecks the current rows and uses the existing handlers.
3. **Flow-inspection dismissal was a hypothesis, not a reproduced bug.**
   Extended the native proof to enter Flow inspection from a retained terminal
   and require that same first responder after dismissal. It passes without a
   production focus change. AppKit's natural sheet dismissal already preserves
   that responder; no speculative focus machinery was added.

## Evidence matrix

`gap` includes deliberately later slices and missing configured acceptance; it
does not imply every row is a regression in the navigation implementation.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| 1: palette/input | Dispatched ⌘K, search, arrows, Return/Escape; exact destinations; retained PTYs | One window/repository palette, Task detail dispatch distinct from sidebar drill-down; search and Flow inspection share a sheet | Native fixture with two owned cat PTYs; search isolation, draft/child echo, Task and Wave selection, same surfaces, restored responder | Local pass; repeated Session activation, stale-row changes and two mounted production windows remain gaps |
| 1: inspection authority | Opening details never starts work | Exact lookup uses shared Task/roadmap projection and retained selected evidence | Actual CLI `exact_task_roadmap_retains_history_without_starting_work`: no active PR, unchanged Run/event counts, unstarted/historical/missing identifiers | Pass |
| 2: exact URL identity | Repo-qualified lookup; all exact collisions offered; unavailable distinct from missing | Typed one-segment parser; `roadmap --task`; generation-fenced model lookup; weak-window router and pending cold URL | Two CLI exact-lookup tests; parser/model/router fixtures | Local pass after scope repair; Launch Services cold/warm installed `open` remains gap |
| 2: asynchronous navigation | A late link cannot replace a click; only intended window moves | Selection invalidates destination generation; router targets one registered window | Existing barrier and two-window receiver fixture | Local pass; two links racing a click through mounted production windows remains gap |
| 3: templates | Preserve repeated composition uses, disclosure and both return edges; Project default on Wave page | Catalog inspection still draws the existing flat graph | `engine/flow.rs::expand_with_chain`, `FlowCatalogInspector`, catalog DTO | Gap: next independent slice |
| 4: invocation/attempt | Exact current attempt and earlier Runs in node, line and Session chip; source-independent history | Existing Task-level provider/time selection remains; parent Run lacks position/attempt projection | `TaskFlowView.runningProvider`, `session.rs::Run`, `PinnedTaskFlow` | Gap: parent integration required |
| 5: room/bind | Null-Task orphan rule, all tiles, one native mount, permanent exact-target confirmation | Existing inventory/grouping unchanged; Session CLI has no Bind | `WorkspaceProjection`, `SessionsView`, `SessionCommand` | Gap: no decorative room or interim writer added |
| 6: races | Current-Run replacement, competing bind, old poll, repo switch, Undo preserve identity | Navigation generation exists; bind lifecycle does not | Source and navigation race fixture | Gap for all bind races |
| 7: configured acceptance | Candidate installed app, selected Home/IDs, live captures at both widths, Jack's walkthrough | Local CLI fixture and AppKit/cat proof only | No installed promotion, Launch Services demo, provider interaction or live captures performed | Gap |
| 8: consolidation/deletion | Shared reader/action owners; remove obsolete presentation when replacements exist | No new storage or resolver authority; palette reuses existing actions; historical selection uses existing evidence | Negative source review below; architecture inventory | Local ownership pass; final deletion and inherited Chapter owner-map gap remain |

## Negative architectural proof

Followed the new route from `LoopflowApp.handleDeepLink` through
`WorkspaceLinkRouter`, `PodiumModel.openTaskLink`, `RegistryQuery.taskDestination`,
CLI dispatch, `wave_tasks` and `snapshot_task_detail`. Exact reads neither invoke
Task preparation nor resolve Work through the launch API. They read cached PM and
registered Task records, returning the same roadmap shape. No Task-link database,
Session mirror, provider launcher, Started writer or ancestry mutation was added.

Palette Task entries and URLs converge at `openTaskDestination`; existing sidebar
single-Session drill-down stays in `openTask`. Retained historical Task evidence
lives in `WorkspaceNavigation.selectedTaskEvidence`, outside current plan membership.
Search state/recents and terminal ownership have separate lifetimes. No new surface
pool or cross-window native view owner exists. The removed `palettePresented` and
`inspectedFlow` fields have no remaining source references.

The still-reachable four-source Session projection, Work/path DTOs, unmatched-roadmap
classification, reverse lookup, flattened template and Task-level running line
are explicitly deferred dependencies in this slice. They are not replacement
adapters introduced here. Searches confirmed the parent CLI still lacks Bind,
and Run lacks node/iteration/attempt order. Removing their current consumers now
would remove capability. The next slices must replace these paths, not count this
review as full-design negative proof.

The architecture checker reports **32/33 SQLite owners**, missing `wave_chapters`;
the other seven inventories pass. This reproduces the inherited parent map gap.
It is not a green architecture gate and this Task does not repair the parent's
storage conversion by relabeling its documentation.

## Verification and limits

Logs: `.lf/tmp/workspace-navigation/review-*`. Resource preflight passed at
89.5 GiB free before execution and 89.4 GiB after builds. Executed commands use isolated LF authority, four-worker limits,
nice +10, serial test execution and a 900-second process-group timeout.

- Before repair: duplicate-identifier CLI test failed with two repositories
  instead of one in the scoped response (30.9 seconds including build).
- After repair: both `status_tests` tests matching `exact_task_roadmap` pass
  (32.7 seconds including build, 9.74 seconds executing).
- Extended `SessionChromeProofTests/paletteRetainsTerminalInput` passes before
  any production focus edit (8.0 seconds including build, 4.26 seconds executing).
  Uses Ghostty's timer-renderer fallback; does not measure compositor latency.
- Final Swift filter `WorkspaceDestinationTests|SessionChromeProofTests/paletteRetainsTerminalInput`
  passes **8 tests** (8.9 seconds including build; 4.22 seconds executing),
  including the historical-action regression. Swift compilation used three
  workers while the serial Rust scope matrix ran.
- All-target Clippy with warnings denied passes (86.5 seconds); formatting,
  Swift platform boundaries and whole active-PR whitespace pass.
- XcodeGen and signed ad-hoc Xcode `build-for-testing` pass (158.0 seconds),
  including bundled CLI helper compilation. This builds the app and test runners;
  hosted UI tests were not executed and the installed app was not replaced.
- The prescribed scope-consumer command passes **12 tests**: 5 in
  `global_commands`, 3 in `wave_resolution_matrix`, and 4 in
  `wave_resolution_tests` (789.1 seconds including build, within the 900-second
  limit). The full command/environment matrix took 711.01 seconds; it completed
  without a timeout or suppressed case. Together with exact lookup, **14 Rust
  tests** passed in this review.

The real CLI tests use synthetic planning in disposable Homes. The native proof
uses fixture transport and real owned cat PTYs. Neither establishes installed
provider behavior, real-Home migration or Jack's acceptance. No live Home was
migrated or populated to manufacture a demonstration.

## Remaining work

Continue the shared template-resolution/disclosure slice, including the current
Project default on the Wave page. Preserve template/capture separation and do not
infer composition boundaries from equal parent names. Integrate room/bind and
attempt views only after the parent's explicit contract checklist passes.

Before treating navigation as accepted, exercise actual Session selection and
stale inventories, mounted multi-window links including two links racing a click,
and installed cold/warm `open` against the candidate. Keep the configured
1440×900/1100×800 walkthrough and Jack's verdict owed for the complete Task.
Do not publish from this review: applicable navigation and configured acceptance
claims remain incomplete. No landing, completion or external messaging occurred.
