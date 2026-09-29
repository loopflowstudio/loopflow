# Execution model concept review

LOO-298 · 2026-09-29 · Review for Jack Heart at `c928d261c`.

The accepted Exec / AgentSession / FlowSession model remains coherent. The latest
slice improves ownership without requiring another product object. Implementation
is not code-complete: runtime loop children, final historical consumers and
discovery, preservation proofs, and legitimate native decision retry remain open.
This autonomous review does not substitute for Jack's review of the complete
implementation or select a Flow edge.

## Usage and the model it requires

The existing [Session usage](../docs/lf.md#sessions) gives the right normal path:

```sh
lf -b implement
lf session list --interactive false --task LOO-298 --json
lf session connect SESSION
lf session rename SESSION 'Parser review'
lf session bind SESSION --task LOO-298
lf flow resume FLOW_SESSION
```

The list filter selects attributed conversations; a launch without Task context
needs an unfiltered lookup before binding. Rename, binding and later connection
refer to the same conversation. A Flow resumes its captured progress independently
of subsequent template edits. These are usage requirements, not a claim that all
provider and Desktop recovery paths have passed.

| User action or fact | Identity and owner | Boundary to preserve |
| --- | --- | --- |
| Run an `lf` command | Exec row; journal admission and completion | One actual process; observed command outcome, causal parent, no inferred signal authority |
| Find, name, bind or continue an agent | AgentSession; admission/publication, connect and binding APIs | Stable conversation, immutable input and attributed history; one current driver; provider generation distinct from driver generation |
| Continue a Flow | FlowSession; captured graph and shared Task/taskless driver | Exact selected successful Session completion or mechanical result; claim/version fences; review feedback alone never advances |
| Repeat a runtime loop | Child FlowSession for that pass | Parent waits; retry keeps child; next pass creates another child; template expansion creates no child |
| Start Task work or assign a conversation | Task's first Started timestamp | Reservation/bind writes once; observation does not start work; prior usage retains its recorded owner |
| Rotate a Chapter | Existing Linear Projects and Tasks | Fresh statuses and stable identities; retain started work and unknown evidence |

Recovery must distinguish a dead `lf` driver from a surviving provider engine.
Connection should reuse the existing engine where possible; explicit replacement
preserves the conversation and must not terminate a shared engine's sibling.
Successful command exit, successful agent turn and completed Flow are three
different facts. A captured input is evidence beneath a conversation, with no
independent resumable lifecycle.

The docs already express this target. No broad prose rewrite is needed for this
review. Final consumer changes must update command examples in `docs/lf.md`,
the centralized cutover status and ownership map in
`docs/architecture-reference.md`, affected Desktop documentation and generated
docs together. Keep `flow list/show` about templates when adding saved-Flow
discovery; the private proposal's `sessions/inspect` names still need collision
review. No new skill or policy layer is required by this slice.

## Findings grounded in the current source

**Live observation now follows conversation identity.** The active reader uses
AgentSession plus existing Exec/native process evidence. SQL selects observed
Exec/PID/client candidates before decoding; no replacement durable observation
owner is needed. Preserve exact process-start comparisons and unknown historical
engine identity. Cold native-client filesystem discovery still scales with
retained input directories; the warm SQL result does not measure that cost.

**Terminal mounting now separates view lifetime from terminal lifetime.**
`GhosttyTerminalRepresentable.makeNSView` creates a fresh `GhosttyTerminalMount`
around the pooled terminal. `updateNSView` changes it only while that mount still
owns it. This explains the extra view: SwiftUI can retire a mount after the
terminal moves, while the workspace retains the surface and unfinished input.
The recorded test-only failure shows the departing host clearing focus after
reparenting; five focused tests pass with the ownership check. The original
hosted detached-terminal observation remains unresolved until checked on the
repair. The earlier overlapping-mount test passed before repair and supplies
no red/green proof of that detachment mechanism.

**Runtime loop ownership is missing behavior.** `durable.rs::FlowSession` and
`engine/invocation.rs::QueuedInvocation` have no runtime parent reference.
`engine/execution.rs::NestedCursor` contains only XOR; returns remain cursor
counters. This does not implement the child-per-pass contract in
[Flow execution](../docs/architecture-reference.md#flow-execution). A parent
filter, graph projection or another passing XOR test cannot fill this gap.
Implement it through the existing Flow owner and transaction, retaining one
managed Task pointer; do not infer parentage from names, cwd or template expansion.

**Historical views still make callers understand the retired Run model.**
`lf/commands/runs.rs::collect_runs` obtains conversation snapshots before applying
its cap. `store/sqlite/sessions.rs::conversation_inputs` reads each selected
Session, including request detail, then constructs a compatibility `RunSnapshot`.
`run_record.rs::conversation_snapshot` still emits string subjects. This is a
remaining public representation, not evidence that runtime Run rows still exist.
Replace these consumers with typed Session history and references to actual Exec
and Flow results. Keep all unterminated records and complete explicit Task/parent
drills; preserve original attribution, failures and usage missingness. SQL already
excludes transcript events from summaries: full transcript hydration is not the
observed problem. Avoid building an optimization owner for the superseded wire.

**Desktop paging must preserve the complete workspace.**
`RegistryQuery.sessions` currently requests `--limit 0`. Changing that call
alone would let partial inventory remove retained panes during reconciliation.
Introduce paging and reconciliation together, keeping bind/rename, off-roadmap
identity, surface, draft, companion and focus behavior. Measure cold/warm CLI
startup, query and payload work separately on the final readers.

**Captured-input naming remains an explicit review finding.** Jack's newly
recorded [naming feedback](concept-review-findings.md) identifies `RunId`, input
fields, unused wire `SessionRecord.run_id`, and the distinct journal TraceId
meaning. Retained `run_…` selectors must continue to resolve. The replacement
type/field name, newly minted prefix and wire disposition remain unselected;
this review does not choose them or conflate input evidence with an Exec.

## Evidence limits and unresolved choices

The [slice review](parallel-execution.md#slice-review-active-session-ownership-2026-09-29)
retains the exact proof scopes. This review inspected source and existing logs;
it ran no behavioral suite. `review-flow-attribution.log` records one passing
public launch/reader case after correcting taskless review inheritance.
`review-terminal-stale-update-red.log` records the focus failure;
`review-terminal-stale-update-green.log` records five passes in 8.856s.
Prior hosted failures and leaked-handle observations remain evidence. No current
hosted-green or configured Desktop result is established here.

Supervisor's scope reconciliation prompted a source/assertion review of early
Exec observation. `with_process` observes early exits through an existing ledger;
install/screenshot observe before dispatch. The three recorded hosted public
tests cover exact help/version/parser exits, no Session or schema creation,
preserved incompatible bytes and distinct screenshot child ancestry. Section 3
of the remaining matrix now marks that boundary implemented, retaining all-entry,
live-installation and signal limits. No duplicate implementation or test run is
needed solely to reconcile this stale checklist. The auxiliary history launch's
401 does not establish a global Codex outage; the supervisor reports successful
text from the current managed Run. No account or store change follows.

The released Flow-discovery proposal remains unapplied/uncompiled. Its metadata
reader cannot supply missing runtime children. Review repository observation
inside the insertion transaction before integration. The released-populated
import bridge is also a proposal, not an executed upgrade: it must run one real
public-binary test and its materialized-copy counterpart with effective provider
isolation. Keep the separate interrupted-import, SQL/file disagreement, native
recovery and full preservation obligations.

Legitimate Codex decision retry remains a failing requirement. Ordinary automatic
retry now retains both outcomes and usage with zero runtime Runs, but failed
native threads retain stale tool authority; rejecting the old child also rejects
the legitimate successor. Successful-idle environment refresh does not repair
that path. The [native tradeoff](native-turn-retry-tradeoff.md) retains the failed
fresh-engine probe and shared-engine constraints. Jack's existing decision
conversation owns the in-turn decision interface versus successful-turn result
choice. Do not silently disable retry or introduce a provider fork/proxy.

Prospective binding remains the recorded conservative implementation assumption,
not a new Jack decision. The complete [remaining matrix](remaining-work.md),
[import contract](import-preservation.md) and [Chapter obligations](chapters.md)
remain binding, including configured provider/Desktop acceptance, incident
dispositions and the separately authorized real-Home conversion procedure.

## Smallest next implementation and proof

Make runtime loop passes real children under the existing FlowSession owner,
then expose their identity through the saved-Flow reader. The distinguishing
proof must show parent wait, failed-child retry retaining identity, exact
successful completion advancing the parent once, and another pass creating a
different child. Exercise Task and taskless ownership and reject stale settlement;
template composition and XOR must remain child-free in this runtime sense.
Retain interruption recovery in the transaction proof.

Finish historical consumer replacement and reviewed discovery/import integration
against the full matrix, carrying the naming finding and native decision choice
without guessing their resolution. Only the complete implementation and its
usage/ownership/deletion evidence can support the requested review with Jack.
