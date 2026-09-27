# Parent integration assessment review

2026-09-27 · LOO-303 · review-slice · reviewed HEAD `5dea707d6`

The [parent assessment](workspace-parent-contracts.md) passes this bounded review.
It identifies a real dependency and the available integration source; it does not
mistake new tables for a usable consumer contract. No new product defect or
executable repair was found. Attempts and room/bind remain pending LOO-298's
owner/consumer conversion. The complete Task is not ready for publication.

Reviewed the active diff from `c832aaede` through `4573a669a` and the supplied
compression note, preserved with `lf commit` at `5dea707d6` before this review.
The [approved design](workspace-ux-on-data-model.md),
[interactive correction](workspace-ux-review-feedback.md), Task directive and
Jack's light-only/no-teardown steer govern. Only scratch notes differ from the
last executable revision `792a3ce40`. This review does not reopen the resolved
folded-target or recursive keyboard findings.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Available integration source | Name the actual parent source and supported integration route | Recorded publication `ca1be1116`; local committed parent `d814eb617`; rebase selects parent tracking branch | Fresh `lf status infrastructure --json`, local ref reads and `lf rebase --plan` | Assessment pass; no integration applied |
| All-kind Session/current Run | Stable conversation, member history and current Run for every kind | SQL owns Task reviews and local-tip interactive Sessions; Ask and taskless Flow remain separate at that revision | Pinned `session.rs`, `human_session.rs:575`, Session store and fixtures | Dependency gap |
| Typed ancestry and historical destinations | Shared Task/Wave/repository ancestry; bound history survives current-plan absence | Typed Run parents exist; public Session retains Work/path; Run filtering scans manifests. Child exact Task reader is implemented | Parent `runs.rs:76`, `human_session.rs:213`, `commands/runs.rs:101`; child exact lookup source and prior CLI proof | Partial; all-consumer ancestry gap |
| Permanent bind | One exact-target confirmation fenced to Session's selected Run | SQL assignment constraints exist; no Bind command/action or confirmation transport | Complete parent `SessionCommand`, `SessionActionKind`, assignment draft | Dependency gap |
| Shared Started | Set-once first-assignment timestamp; all surfaces read it | Trigger exists; current reader still combines events/generations and roadmap adds PR evidence | Parent `chapters.rs:96`, `commands/run.rs:912`, roadmap projection | Dependency gap |
| Position attempts | Ordered Runs and authoritative current attempt feed node, running line and Session chip | Parent storage allocates ordinals and fences selection; public Flow/Session DTOs do not expose the required attempt history | `runs.rs:144,196,234`; repository-wide `position_runs` caller search; five fixture comparisons | Storage present; consumer gap |
| Retained invocation/taskless history | Captures and runtime children remain inspectable; common taskless owner and legality | Task capture is retained in SQL; public finished record omits graph; taskless driver still writes `position.json` | `TaskFlowRecord`, `PinnedTaskFlow`, `flow_run.rs:108,136` | Dependency gap |
| Done When 1–3: navigation/templates | Exact keyboard/link destinations, retained input, recursive template disclosure | Existing independent slices unchanged | Prior CLI, model and mounted proofs in the navigation/template/return reviews; verified unchanged executable bytes and retained logs | Local evidence retained; installed links/appearance remain gaps |
| Done When 4–6: attempts/room/races | Exact A/B/C attempts; all null-Task orphans, one mount, universal bind and race handling | No dependent UI introduced; old grouping, hidden host and Task-based running line remain | Current source and parent contract audit | Pending implementation; no decorative replacement |
| Done When 7–8: configured acceptance/deletion | Installed providers/drafts, both capture widths, Jack's verdict, delete replaced paths and reconcile docs | No installed activation or final replacement/deletion in this slice | Scope/diff audit; prior architecture receipt still fails on `wave_chapters` | Gap; no publication |

## Fresh observations and retained evidence

Read-only checks at approximately 09:11–09:13 UTC:

- `lf status infrastructure --json` succeeds and records LOO-298's active PR
  `pr_64160469a52645eaafa913a5e024d2db`, GitHub #1296, phase `open`, publication
  head `ca1be1116d704a4d908d473c2434ef59fd23261f`, with no recorded merge.
  This is shared recorded state, not a new remote-head probe or proof that no
  independent parent contribution is running.
- The parent's local committed HEAD remains
  `d814eb61775ed322fb7e459e1f2aba0daf9665e5`; the local tracking ref remains
  `ca1be1116`. `lf rebase --plan` selects
  `origin/jack-heart/data-model-one-table-per`, fork base `c832aaede`,
  `clean_authored` / `direct_rebase`, 17 unique commits and 39 changed files.
  This confirms the route after the supplied upstream correction without
  changing tracking or applying a rebase.
- Verified all 27 retained parent file hashes against both their receipt and
  the immutable Git blobs at `d814eb617`. All five Session/Flow/execution fixture
  blobs parse and remain byte-identical between `ca1be1116` and `d814eb617`.
  No public attempt contract is hidden in a fixture-only change.
- Read the retained `return-review-swift.log`: nine tests in five suites pass,
  including dispatched disclosure/input isolation and owned-PTY retention.
  `return-xcode.log` records `TEST BUILD SUCCEEDED`. `git diff 792a3ce40
  --name-only` contains only scratch notes. Reuse these receipts within the
  [return review](workspace-return-review.md)'s local fixture/PTY and compile
  limits. Earlier exact CLI and resolver receipts retain their original scope.

Fresh state/plan output is under `.lf/tmp/parent-contract-review/`; pinned source
and hashes remain under `.lf/tmp/parent-contract-assessment/`. The original
assessment's dirty Ask snapshot remains dated evidence. This review neither
reassesses those moving bytes nor calls them a tested integration revision.
No product tests, builds, resource recovery, native rendering or installed
interaction were repeated for this notes-only slice.

## Source and ownership judgment

The assessment correctly separates storage from public projection. In the pinned
parent, `insert_run_in` allocates attempts in the write transaction;
`select_attempt_in` checks invocation version and pending Session identity.
`position_runs` reads that stored order. Its only domain callers found are tests;
the remaining references are Store forwarding and the implementation itself.
`PinnedTaskFlow` has no ordered/current-attempt fields, and Session membership
has no attempt ordinal. Existing `TaskExecutionSnapshot.run_id` is acknowledged;
it does not supply position history or the three-surface contract by itself.

The common-owner removal is not complete: `human_session::list` still dispatches
to Ask and taskless Flow sources, `flow_run` reads/writes `position.json`, and
the Run reader still scans manifests through WorkCatalog. These are existing
parent dependencies, not new adapters added by LOO-303. They prevent treating
the full target architecture as delivered. No second Session store, bind writer,
Swift attempt counter or competing invocation owner was introduced here.

On the child path, palette and Task links converge at `openTaskDestination`;
historical selected evidence stays outside current-plan membership. Template
composition comes from the existing resolver and flattening path; the original
graph supplies return detail while the folded projection supplies arrow placement.
These owners remain coherent. The original/projection distinction and separate
terminal input eligibility are supported by the recorded counterexamples.

Current `unmatchedSessions`, reverse Session lookup, opacity-hidden checkout
host and Task-based provider/time selection remain reachable. Deleting them now
would remove behavior before the approved replacements exist. Their retention
is an explicit unfinished boundary, not negative proof of a completed cutover.
The assessment advances the full design by preventing an incompatible interim
UI; it delivers no additional user-visible behavior on its own.

## Next useful boundary

Resume from a parent owner/consumer conversion revision with updated fixtures
and proof. Reassess the existing eight-contract checklist, integrate through
`lf rebase`, preserve child historical lookup, and extend the shared attempt
projection where still needed. The first dependent proof remains failed A then
current B at one position with independent Task Run C concurrently active:
node detail, running line and Session chip all identify B as attempt 2, while
selecting A retains A. Do not infer ordinals in Swift or rebuild parent storage.

Keep loop/restart/child and source-independent history, unknown history, every
null-Task orphan (including Wave-only, non-bindable reviews and shared shells),
one native mount, all bind confirmation/race cases, installed cold/warm links,
configured providers/drafts, live 1440×900 and 1100×800 captures, Jack's verdict,
final deletion/docs and the inherited architecture failure as open obligations.
Preserve the sidebar Session shortcut and overall-monitoring placement for
non-Task execution. No new user decision is required by this assessment.

No publication, Task completion, live migration, external message or Flow
navigation decision occurred. Whitespace and this note's local-link checks pass;
those checks validate the review artifact, not the product acceptance gaps.
