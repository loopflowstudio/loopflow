# Wave-only workspace review — 2026-09-24

The integration advances the accepted repo → Wave → Task → Session design.
Project remains internal chapter identity; Full presentation does not restore a
Project tier. Two navigation defects were reproduced and fixed. Publication is
still pending the core discovery, measurement and configured acceptance work.

## Findings fixed

- Historical-reference resolution ran in an unowned asynchronous Task. A slow
  result could replace a newer Task selection, or open another repository's
  history sheet after switching repositories. Podium now retains and cancels
  that lookup on subsequent navigation, and checks cancellation after the read.
  The regression holds the actual RegistryQuery response, changes navigation,
  releases it, and awaits lookup completion. It also proves the uninterrupted
  history link still opens the correct Wave and chapter.
- Selected Task evidence was captured only on selection. A successful refresh
  updated visible details, but a subsequent chapter-transfer gap reverted to
  the original selection-time data. Each roadmap publication now updates the
  existing repository navigation's selected Task evidence when present. Missing
  membership retains that latest observation. No new planning record or writer
  is introduced. The regression changes the selected Task's name before its
  membership disappears and requires the updated name throughout transfer.

Both tests fail before correction: four assertions fail across the stale Task
case and the Task/repository navigation cases. The original implementation's
transfer test did not include an intervening planning update.

## Evidence matrix

| Claim | Planned and implemented behavior | Proof | Result |
|---|---|---|---|
| One hierarchy | Direct Wave Tasks; compact and flat Sessions use the same projection | Current source/DTO comparison; unchanged integration outline receipt | pass locally |
| Stable Session identity | Required Run ID, shared Wave ancestry for historical Project Sessions, exact typed Task join | Rust/Swift mirrors, shared fixtures and prior ancestry/action receipts | pass at contract scope |
| History navigation | A delayed link cannot override later navigation | New held-response regression, including successful history lookup | pass after correction |
| Transfer details | Retain latest observed Task through missing membership | Reproduced stale-detail failure and corrected regression | pass after correction |
| Shared workspace | Chapter change preserves Monitor attribution, original Session draft and responding companion | Fresh mounted SessionsView/Ghostty test; same Task Work, Session Run, panes and surfaces | pass, fixture reads and real PTYs |
| Active monitoring | Exact live Task Runs, readable stale/unknown state, one reader | Existing ActiveRuns projection and Monitor source; native fixture positive row | configured positive population and bounded discovery gaps |
| Two performance experiences | Repeatable hierarchy/workspace outcomes and comparable samples | Concurrent runner/documentation inspected; full receipt still pending | partial implementation; no baseline claimed here |
| Complete acceptance | Human confirms simplified composition; external edit/trials and published budgets | No new human demo or external mutation | gap |

Focused command:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel \
  --filter 'PodiumModelTests|TaskMonitorProofTests/monitorRetainsNativeDraftAndCompanion'
```

**19 tests in three suites pass**, including all three historical-navigation
cases and the native chapter-transfer path. [Before](wave-review-evidence/before.log),
[after](wave-review-evidence/after.log), [source receipt](wave-review-evidence/sources.json).
No review-owned executable edits followed. This is the closest safe local path:
the chapter read/move is simulated, the window and cat PTYs are real. It proves
neither live provider migration nor human acceptance. No production migration,
installed-app replacement, Session movement or live chapter rotation was attempted.

## Source scope and ownership

Recovered the complete tracked base-to-worktree patch in
`/tmp/loo291-review-wave.patch`, approximately 21.4 million characters. Splitting
on actual diff headers yields 690 file sections: 391 identical to review 20,
299 changed/new and none removed from that review's section inventory. Binary
patch bodies are retained. Untracked performance files and integration receipts
were inspected separately; the patch is not represented as covering them.

Used the prior canvas review and incoming chapter review for unchanged source,
then inspected integration seams, merged DTOs, navigation, launch, Task activity,
historical references, Session projection and retained workspace paths. The source
classification records 130 non-scratch files identical to chapter snapshot
48622b569, 33 identical to the prior canvas checkpoint, 41 combined/new files and
11 deleted paths. This is provenance accounting, not a fresh independent review
of every unchanged historical implementation. The sibling's documented
objective/metric-target correction remains excluded.

Negative searches find no WorkspaceProject, RoadmapProject, PodiumConsole or
removed Session scope/action-policy wrappers in the reachable Mac/model path.
Podium has one caller each for roadmap, Sessions and active Runs; PodiumView
creates one root SessionsWorkspaceRegistry. Monitor reuses the same checkout
multiplexer, focus and native surface ownership. The Project Work enum remains
historical provenance, not an ordinary conversation target or navigation node.
Chapter rotation remains the membership writer; ordinary Task saves do not own it.

The active Run reader still enumerates `record_dirs` to discover native-client
receipts. It does not replay historical events, but its discovery cost is not
bounded independently of accumulated Run directories. Monitor explicitly asks
for Refresh; automatic shared refresh still awaits that cost correction.

The concurrent performance runner measures forced native bitmap capture with
text verification and retained PTY responses. Its documentation distinguishes
observer overhead from compositor presentation and frame hitches. Full samples,
production phase correlation, scrolling during refresh and scoped budgets remain
open. This review did not run or certify that benchmark. Its recorded five-pane
clipping observation remains a composition limitation.

During review another writer removed the redundant `routing_project_id` field
from Rust/Swift and fixtures. Preserved it and inspected the matching removal.
The final Swift build includes that source and the updated native transfer test;
the test still changes `project_id` and requires the same Task/Run identities.
Earlier Rust receipts remain attributed to their original hashes. No new Rust
pass is claimed for the concurrent reduction. `git diff --check` passes.

While this report was being written, the other writer checkpointed the shared
implementation as `03105f382` (`lf commit: compress`), including these two fixes
and their tests. All final review source hashes still match afterward. This
review did not create that checkpoint or publish it.

## Next slice and disposition

Reconcile the complete committed objective/metric-target correction, finish
bounded discovery and shared refresh, and finish the two measurement journeys
with explicit rendering/hitch limits and comparable source-stable samples.
Then demonstrate the simplified composition and positive activity in the
configured app. Preserve the original external-work/edit obligations and one
later optimization Task per experience; these local checks do not satisfy them.

No publication or Task completion. The invoked
[review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md) requires
“When all applicable `Done when` claims hold and the slice is coherent, publish or
refresh the Task PR with `lf pr publish`.” The remaining core measurement and
configured-acceptance claims leave that condition unmet. LOO-293 and both sibling
worktrees remain untouched.
