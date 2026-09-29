# Concept review: conversation history and Flow results

LOO-298 · 2026-09-29 · Autonomous review of `537e7924d`.

The runtime-child slice fits the accepted ownership model. LOO-298 is not
code-complete: Jack Heart's latest captured-event and structured-result decisions
remain unimplemented, and history/discovery/import obligations remain open. This
is evidence for the saved Flow's decision step, not Jack's complete-implementation
review, acceptance, or a navigation decision.

## Experience and ownership

Jack should be able to find a headless conversation, connect to it, rename or
bind it, and continue it after its original command exits. Its identity, feedback
and native history survive. A repeated Flow pass has its own saved progress;
retry resumes that pass without duplicating it or advancing its waiting parent.

| Action or fact | Product owner and interface |
| --- | --- |
| Find, connect, rename, bind and resume a conversation | AgentSession; existing Session commands and Desktop actions converge on that identity. |
| Reserve input before launch | A captured event in that Session's history, selected by sequence; publication is later evidence. This is Jack's target, not current code. |
| Observe a command's exit or its causal children | Exec; actual process identity and terminal receipt, independent of provider completion. |
| Continue saved Flow work | FlowSession; captured graph, selected boundary and fenced settlement. Task's managed pointer stays on the root. |
| Repeat work | Runtime child FlowSession; parent waits, failed retry retains the child, successful return advances once. Template composition and XOR alone add no child. |
| Choose a decision or route | Typed output of the selected successful agent turn, validated and consumed from Session history. This is Jack's target, not current code. |

The distinguishing recovery example remains: an agent fails in Session S under
Exec A; Exec B continues S and records a successful turn. The Flow consumes that
exact success once. Both outcomes and their usage remain visible. A later
conversation turn cannot rewrite the consumed result; command success cannot
substitute for provider or Flow success.

## Findings for the next implementation boundary

### 1. Deleting the input catalog must preserve evidence without inventing Sessions

Current `AgentSession.input_id` still uses `RunId`; reservation/replacement and
Flow publication compare that value. `agent_session_inputs` also retains opaque
historical SQL with nullable conversation membership. The
`retain_input_sql_evidence` migration moves old rows there before dropping
`runs`, and uses those rows to protect historical ancestry and Started.
`historical_session_inputs` explicitly returns absent conversation membership
for unclassified evidence (`store/sqlite/sessions.rs:246`).

Jack's event decision supersedes the earlier assumption that this table would
remain. A blind table-to-event conversion cannot preserve an unclassified row:
Session events require a real Session. Do not fabricate a conversation, discard
the row, or retain the input catalog under a new object name. The implementation
design must name the retained import-evidence destination and preserve exact
payload, selectors, missing membership, ancestry constraints and Started before
dropping the table. Mechanical evidence with an established Flow belongs in its
history; genuinely unclassified evidence remains explicitly unresolved.

For known conversations, write the captured event at reservation and select it
atomically. Preserve comparison against the prior capture across retry and
publication; a sequence must identify evidence, not acquire a new resumable
lifecycle. Inventory artifact-directory readers before replacing their keys.
Remove `RunId` and `SessionRecord.run_id` across Rust, Swift and fixtures together;
keep stored `run_…` selectors and the separate journal TraceId boundary.

Proof must include reservation/publication interruption, same-Session replacement,
old selectors, unclassified SQL and populated draft/canonical upgrades. Existing
final-schema seeds cannot discharge those preservation cases.

### 2. Typed results must replace the decision authority path through recovery

Current CLI `control` still calls `record_flow_decision`/route, and
`require_turn_authority` checks the agent-issued command's selected-turn token.
`checkpoint_flow` preserves the separately written verdict/route before consuming
completion. These are concrete deletion targets, not the selected final model.

Keep the existing Flow version/claim and exact successful-completion transaction.
Validate the declared output against the captured boundary's verdict or allowed
routes, retain it in Session history, and consume that result there. Validation
failure and provider failure are different observations. A bounded validation
retry stays in the same conversation and retains earlier evidence; it must not
turn an earlier invalid or stale success into authority. Delete the old decision
CLI, writers and teaching text when the replacement lands. Jack explicitly
required no decision alias; outright router-command removal is recorded as the
supervisor's corresponding assumption.

Recovery needs a deliberate treatment of saved instructions. Task decision prose
is assembled in `controller/task/mod.rs:521`, while the default XOR router's
captured skill content embeds `lf flow route PATH` in `engine/flow.rs:954`.
Updating today's catalog alone leaves that saved instruction. Preserve captured
graph/policy and explain how historical decision instructions cross the change;
neither silently recompile the saved Flow nor keep a second writer to hide it.

The required proof is an invalid output followed by a valid result in the same
Session, bounded exhaustion, old/stale result rejection, and exactly one parent
continuation for Task and taskless execution. Retain shared-engine sibling and
failed-thread/successful-idle evidence. The old legitimate Codex retry failure
remains open until the selected replacement works; no new provider machinery or
retry-policy exception follows from this review.

### 3. Final history must discover evidence that lacks an input relationship

The released history proposal is unapplied and uncompiled. It removes
`RunSnapshot`, string-subject joins and post-hydration limits, but adds input-based
projection types and retains `RunId`. Rebase its useful reader work onto the
captured-event decision before integration; remove wrappers without an independent
consumer need. Its handback explicitly excludes native receipts with no input or
start relation from aggregate discovery. Exact Session history availability does
not by itself satisfy general history/usage discovery. Preserve those receipts
with unknown attribution and usage coverage rather than assigning today's owner.

Desktop paging is coupled to reconciliation: `RegistryQuery.swift:208` requests
the complete inventory, and `SessionsView.swift:158` removes identities absent
from each supplied result. Passing one page into that API would remove retained
Sessions. Change acquisition and reconciliation together, proving selected pane,
terminal, draft and focus retention across page boundaries and failed refresh.

Flow discovery remains a separate released proposal, now able to use real
runtime parentage. Review its insertion-time Git lookup and template/command-name
collisions before integration. Private SQL timings and syntax checks establish
neither public behavior nor the required cold/warm end-to-end measurements.

## Evidence retained by this review

Source inspection confirms that `checkpoint_flow` consumes the selected success
inside its transaction and `checkpoint_in` completes/returns a child or allocates
the next pass there (`store/sqlite/flows.rs:519,645,1346`). The Task root pointer
and shared driver remain; no alternate lifecycle owner is introduced.
Initial forward execution stays in the root; each taken backward edge starts a
child pass. This remains main's recorded implementation interpretation for Jack's
final review, rather than an additional product decision attributed to him.

- Read `review-runtime-repairs.log`: 16/16 pass, including the public taskless
  runtime-child case and publication adoption/promotion recovery. Providers and
  GitHub are scripted; Git remotes are local.
- Reuse unchanged 29 focused runtime checks and 22 materialized ownership/Chapter
  checks recorded in [runtime evidence](runtime-flow-children.md). No new test run.
- Read the successful all-target Clippy log. Reuse supervisor-reported hosted
  Swift success at `067ff0164`, run `36614840195`; it supersedes the earlier
  outstanding terminal-detach confirmation.
- Hosted Rust's 1,917 passed / one failed / 15 skipped / 80 unrun remains its
  actual result. The publication failure has focused repair evidence, not a new
  green hosted matrix. No production edit, build or behavioral check ran here.

The full [remaining work](remaining-work.md), [import obligations](import-preservation.md)
and [Chapter contract](chapters.md) remain binding. Early Exec observation and
runtime-child production are implemented; stale checklist language is not a new
implementation assignment. Prospective usage attribution remains the current
assumption pending Jack's decision. Configured provider/Desktop acceptance,
public released-populated import and canonical-copy proof, incident dispositions,
final checks and preparation of the real-Home procedure remain explicit obligations.
No branch access to the installed Home or promotion is authorized here.
