# Monitor review and Wave reconciliation — 2026-09-24

The human's latest direction removes the public Project tier, including Full
presentation. Target: repo → Wave → Task → Session, compressible through the same
outline to a flat Session list. Task Monitor and Sessions remain pane content in
the existing multiplexer. This changes the next integration slice, not the
required Session-to-Run identity or terminal ownership.

## Observed sibling contract

Read `loopflow.projects/scratch/projects.md`, its questions and UI research,
then inspected Rust `lf/commands/waves.rs` and Swift `RegistryQuery.swift` and
`WaveWorkMap.swift`. The sibling is clean at this observation. Both roadmap
mirrors expose one optional chapter summary, direct Task evidence and unavailable
Task evidence. The design says Wave is the everyday public name; the current
Project is its internal chapter/provider record. Wave owns objective; Project
owns Tasks, KRs and metric targets. A chapter change preserves started Task
identity and execution while retiring untouched backlog.

That design records an outstanding ownership correction: metric targets and
chapter objective presentation still need reconciliation. Source inspection here
establishes the flat read contract, not completion of that correction, a live
portfolio migration, or native integration with this branch. The sibling research
describes older PodiumConsole/Session navigation that this branch has removed.
Copying its whole Swift workspace would restore superseded owners. No sibling
files, runtime, Tasks, branches or live portfolio were changed.

## Bounded correction and proof

Found a Task-choice bug beyond reused pane IDs: focusing Task B's already-open
direct Session while Task A remained selected overwrote A's saved choice. The
new mounted `paneFocusPreservesTaskChoice` regression failed both return assertions
before correction (`/tmp/loo291-monitor-review-focus-before.log`).

`SessionsView.rememberTaskPane` now saves a direct Session only when its existing
shared subject matches the selected Task. No new identity model or pane owner.
The focused command passes all six tests:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel \
  --filter 'TaskMonitorProofTests|TaskMonitorTests'
```

Receipt: `/tmp/loo291-monitor-review-after.log`. It includes mounted Task return,
stale/incomplete observations and real Ghostty PTY draft/companion retention.
These use fixture registry responses, not configured provider interaction.
No executable edits followed this pass. The separate iteration 20 review owns
its configured empty-Monitor receipt; this pass's prepared configured probe was
not executed and supplies no additional configured result.

| Claim | Planned / implemented behavior | Evidence | Result |
|---|---|---|---|
| Task return | Preserve each Task's own saved pane | Reproduced failure; six-test focused pass after exact-subject fix | pass locally |
| Shared Monitor layout | Session, Monitor and shell share split/focus/zoom and native lifetime | Same focused native PTY tests | pass at fixture scope |
| One public planning level | Wave directly contains Tasks; chapter Project internal | Sibling Rust/Swift contract exists; current outline still has Project | integration gap |
| Stable identity through rotation | Current parent changes without replacing Task, Run or terminal | Sibling design; no cross-branch integration proof here | gap |
| Performance | Hierarchy interaction and Task workspace ready measurements | Defined scenarios; runners/baselines still outstanding | gap |

Inspected the unrestricted tracked patch `/tmp/loo291-monitor-review-tracked.patch`
(463 sections, 427 unchanged from review 19), changed model/view paths and new
Monitor tests separately. Source searches retain one `query.activeRuns` caller
in PodiumModel and one production root workspace registry in PodiumView. The
correction reuses `WorkspaceProjection.subject(for:)`; no extra reader, writer,
Session policy or Monitor-specific layout is introduced.

## Next slice and disposition

Integrate the shared chapter binding/read projection and this branch's outline
together. Remove Project selection, compression special cases and ordinary
Project launch targets; do not insert a Chapter row. Wave details must retain
objective, current KRs/targets and full Tasks. Keep unavailable/historical work
reachable, resolve old Project-bound Sessions through shared ancestry, and retain
required `Session.run_id`. Prove a Task moving chapters keeps its selected pane,
Session, Monitor attribution and companion terminal.

Then finish bounded active discovery/shared refresh and both native experience
runners, recording rendered/usable endpoints and retained failures before the
per-area optimization Tasks. The fallback compile, configured positive activity,
human composition acceptance and original external proof obligations remain.

No publication or Task completion. The
[review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md) requires
“all applicable `Done when` claims hold”; the changed hierarchy and remaining
measurement/acceptance gaps leave that condition unmet. This review updates
direction rather than approving the superseded hierarchy.
