# Remaining integration questions and boundaries

- Public navigation is repo → Wave → Task → Session. One current chapter Project is internal. The human resolved this direction; do not restore a Project tier or ask again.
- Rebased onto post-#1281 main `236086c94`; the complete chapter ownership/metric-target correction is integrated. [Integration and pending demo](post1281-integration.md) records passing focused proofs and the configured Home/schema/first-chapter boundary.
- No live chapter migration was applied. Existing portfolios need the deterministic read-only preview and human acceptance before moving or abandoning planning work. Code integration is not migration approval.
- Configured active discovery/shared refresh, the two complete measured UI experiences and the human canvas demo remain core proof. Local stream and both compilation paths now pass. The concurrent performance contribution is preserved; its writer owns its endpoint and baseline claims.
- Environment-only active discovery was rejected: macOS omitted the Run locator for an owned live cat client while exposing it for Python. The [next discovery design](main-view-task-discovery.md) selects one foreground Rust reader retained by Podium, existing receipts, macOS filesystem notifications and explicit cold/rescan cost. See [the counterexample](active-run-refresh.md). The [Rust reader/CLI slice](discovery-implementation.md) now passes ownership,
  recovery, lifecycle and three-population cost proofs, including the ownerless
  capture dependency counterexample. [Automatic Monitor transport](active-runs-stream.md)
  now has local lifecycle, real CLI and native input proof; configured vendor
  activity and human acceptance remain open.
- Discovery defaults need implementation evidence, not another product decision: macOS continuous mode first; one reader per Podium/window/Home retained after first Monitor use; two-second serialized sampling; explicit Retry after transport failure. Prove native event delivery, scan/publication races, lost-event recovery, bounded warm work and cancellation before enabling automatic refresh. Linux continuous mode remains unsupported in this slice; its existing one-shot reads remain available. No missing human answer blocks this handoff.
- No human-selected external proving workflow or exact directive edit has been supplied. Preserve those full-Task obligations without inventing a PM mutation.
- LOO-293 and its worktree remain open. Integrating shared Monitor primitives does not establish completion of its remaining behavior.

Current evidence: [Wave integration](wave-integration.md). Historical decisions remain in the design and earlier review receipts; they do not override the latest hierarchy amendment.


## Implementation handoff — 2026-09-25

[Current design](main-view-task.md) supersedes historical visual proposals.
No unanswered visual question blocks the first native slice. New session belongs
beside the Task title; Session names are automatic and editable. Skill invocation
supplies the initial name; raw Sessions use the historical magical musical animal
generator. Operating guidance may improve generated names while preserving human
names. No new naming model call is needed.

Technical investigations, to resolve in the existing owners during implementation:
- Locate/recover the historical name generator. Current Rust/Swift/Python source
  search did not locate it. `ops/human_session.rs::session_title` currently derives
  interactive titles from context, then skill/cwd or harness; it is not the accepted
  durable generated/manual naming behavior. Resolve title storage/provenance and
  a shared rename operation for Interactive, Ask and Flow Sessions; do not create
  a second Session registry or teach a command before it exists.
- Project exact active/historical Flow membership and occurrence through shared
  Run/Session evidence. The current Session DTO lacks this relationship. Unknown
  membership must not be rendered Independent. Verify pinned Flow definitions,
  authoring search and pause/restart semantics before enabling native controls.
- Read actual comment count/thread through the planning boundary; fixture zero
  is not evidence of an empty Linear thread. Do not rewrite existing descriptions.
- Confirm unstarted Task checkout preparation supports an independent Session
  without starting its managed Flow. Recover read/launch errors without inventing
  started-work or resetting a retained terminal.

The human-selected external trial/edit and performance acceptance obligations
above remain separate completion inputs; do not invent them for this design pass.

## Cycle 1 naming assumptions — 2026-09-25

- History contains a `magical-musical` generator (commit 309575f8e) but no animal list; restored it verbatim rather than inventing animals.
- Ask Sessions keep their bounded question as the generated seed (neither a skill nor raw); Flow Sessions now seed from the step skill instead of the Task title.

## Cycle 2 Flow membership assumptions — 2026-09-25

- Membership is recorded on the Run manifest at capture (`RunSpec.flow`). Wave-resident flows, direct `lf --task X <flow>` runs, replays and Asks record `independent`: they are not occurrences of the Task's managed Flow. Pre-existing manifests project `unknown`.
- A remote Flow Session's title is the step label with `title_source: unavailable`; reading the remote canonical name needs a routed read, not attempted here.

## Observed provider transport follow-up — 2026-09-25

The installed Codex driver left a Run waiting after turn/start was rejected with
`input_too_large` (1,158,726 characters versus 1,048,576). Exact response and owned
cleanup are recorded in `implementation-cycle/continuation-integration.md`.
No provider turn ran. Propagating rejected turn-start responses remains a separate
driver defect; the current dependency correction uses Claude's text-stdin path
and does not claim that defect fixed. No Task was created for this observation.
