# Open assumptions

LOO-298 · 2026-09-28. Settled decisions live in [the contract](data-model-one-table-per.md);
this file contains only unresolved choices or limits.

- **Usage after bind:** supervisor's conservative implementation assumption is
  prospective attribution. Earlier work keeps its recorded owner, subsequent work
  follows assignment. Jack has not selected whole-conversation versus prospective
  usage attribution. Preserve assignment time; an active turn's cumulative usage
  cannot silently move or be split without evidence. This does not block independent
  implementation and does not authorize rewriting historical events.
- **Whole-Flow binding:** outside the selected Session bind operation. A taskless
  Flow and its member Sessions cannot acquire incompatible Tasks piecemeal.
- **Wave without a current Project:** automatic creation on Task start remains a
  proposal, not accepted policy. Report the missing current plan clearly.
- **Every-process observation:** parser/help/version, bootstrap/installation and
  screenshot dispatch need final implementation/disposition. No missing observation
  may be represented as an observed process completion. Ordinary command logging
  failure and required agent-launch rows have different consequences.
- **Proof limits:** local retirement cannot prove absence of remote-Home work;
  native fixtures cannot prove configured accounts, rendered Desktop or installed
  migration. Preserve those limits when preparing acceptance.

No new Jack decision is needed to continue exact Flow history selection, Run
removal, complete import, indexed discovery or coordinated Desktop conversion.
Historical questions, superseded exceptions and unattributed early approvals are
preserved in the [archive](parallel-work.md), not reattributed to Jack.

2026-09-29 · Exec discovery implementation choice: `lf exec list` and `show`
expose the existing process owner. List defaults to 100 rows in the current
repository; `--all` spans repositories. `--after` consumes the prior JSON `next`
object with unchanged filters. This is bounded continuation, not snapshot
isolation or a new lifecycle. Literal search renders stored argv elements with
spaces while returning unchanged raw command evidence. Desktop paging remains
separate unfinished work; a query/model fixture does not establish that consumer.

2026-09-28 · Iteration 10 implementation choice: AgentSession owns its current
captured input and publication. `agent_session_inputs` retains only the immutable
input-to-conversation references required for earlier-input lookup after retry;
it has no result, state, ordinal or resumable lifecycle. Provider outcomes remain
Session events, command outcomes remain Execs. This does not replace the remaining
import-preservation proof or authorize deleting historical inputs.

2026-09-28 · Historical import retains legacy input and provider observations as
subordinate AgentSession history. An imported observation is not a native turn
completion: unknown native thread/turn and Exec identity remain nullable, and
Flow success selection continues to require exact native completion evidence.
Original source/input keys namespace replay comparison; no imported input gains
an ordinal, mutable outcome, or resumable lifecycle. Retain original artifacts
until the complete preservation obligations pass.

2026-09-29 · **Decision interface — asked, unanswered.** Supervisor asked Jack
in the existing control conversation whether a Flow decision should be returned
by the selected successful agent turn and consumed from AgentSession history, or
remain an in-turn `lf flow decide` call. The result-based option is supervisor's
recommendation, not Jack's decision. The retained research compares its interface,
history/recovery and provider costs with native tools and upstream reload repair:
[native retry options](research-native-retry-options.md). No alternative transport,
upstream action or retry-contract change is selected. Existing decision failures
remain requirements, and independent Run removal, import and reader work continues.

- 2026-09-29, SQL preservation: the initial skill-name classification at
  `4682d662a` was disproved by old managed mechanical rows carrying a skill label.
  `provider=loopflow` is operation evidence. Provider-null labels require captured
  node policy or a recorded Session relation; labels alone remain unclassified.
  The populated fixture retains an unknown caller without fabricating a Session
  or Exec. Its destination remains open before table removal.

## Historical SQL destination (2026-09-29)

Main interprets the accepted one-time input mapping/evidence contract as allowing
the existing immutable-input catalog to retain an opaque original SQL payload.
Conversation attachment may remain unknown; such a row is neither AgentSession
nor Exec and keeps the public import failure until classification is established.
There is no mutable execution state or new attempt API. Use that existing owner
to preserve every mapped and unmapped row before removing runs. Current input
relationships stay separate from the frozen historical payload. Migration proof
must retain original foreign-key/ancestry constraints, Started, exact replay,
unknowns and canonical rollback. This is an implementation assumption, not a new
product decision attributed to Jack.

## Runtime-pass entry assumption (2026-09-29)

The captured graph has backward edges with overlapping ranges, rather than static
loop blocks. Main implements each taken Iterate edge as entry into a child pass;
initial forward execution stays in the root. The child returns at the deciding
node; another Iterate there creates a sibling, while an inner edge creates a
nested child. This preserves authored targets and counters and is an implementation
interpretation of Jack's accepted child-per-pass contract, not a new Jack decision.
Source/canonical/public fixtures cover this interpretation; final concept review
must retain it explicitly.

2026-09-29 · **Decision interface: decided.** Jack Heart chose the structured
result in the operating conversation: a Flow decision is returned by the selected
successful agent turn and consumed from AgentSession history. He named PydanticAI's
typed-output logic as the inspiration. The in-turn `lf flow decide` path must not
remain a second writer for those boundaries. This supersedes the "asked,
unanswered" entry above. The captured boundary supplies a native output schema; exact successful output owns settlement.

2026-09-29 · **Captured-input naming: reframed, not decided.** Jack rejected the
candidate names. His principle: an ID is the ID of an object type in the data
model. The open question is therefore what object the identifier names, before
any spelling. No rename proceeds until that is answered.

2026-09-29 · **Decision command: remove.** Jack Heart decided the in-turn
decision CLI is removed outright once the structured result lands; he said it
"felt wrong" already. No alias, no compatibility path. Supervisor assumption, not
yet confirmed by Jack: the router's in-turn command goes with it, since a route
is the same kind of result.

2026-09-29 · **Usage after bind: Jack leans, undecided.** Jack is "slightly more
inclined towards post-hoc attribution and allowing history to change" and said he
could be wrong. Candidate shape offered to him: keep usage events immutable and
derive ownership from the Session's write-once Task at read time, with bind time
marking the pre-bind portion. Prospective attribution remains the implemented
behavior until Jack decides. Do not change it on this entry.

2026-09-29 · **Captured input: decided.** Jack Heart decided a captured input is
an event in AgentSession history, not an object. It is identified by its
`session_events` sequence and names its Exec through the existing nullable
`exec_id`. The `RunId` type and the `agent_session_inputs` side table are
deleted. The Flow's current-input pointer becomes a Session event reference, as
its selected start already is. Stored `run_…` strings stay valid selectors as
import evidence resolving to an event. Jack rejected `SessionExec`: (Session,
Exec) does not identify an input, and the contract excludes that object family.
Supervisor assumption, open to Jack: the event kind is named `captured`.
Unverified: how many readers depend on artifact directories keyed by the old
string.

2026-09-29 · **Operating assumptions under Jack Heart's autonomous goal.** Jack set
the goal "get all the way to code-complete autonomously" and did not answer two
open questions. The operator proceeds on these, both reversible, neither
attributed to Jack as a decision:
- The router's in-turn command is removed together with the decision command;
  a route is the same kind of typed result.
- Usage after bind stays prospective, as implemented and proven. Jack's lean
  toward post-hoc attribution, read from the Session's Task at query time, is
  carried to the concept review as an open finding, not built now.

2026-09-29 · **Blocked feedback retained.** The consumer inventory identifies
`lf flow blocked` as a keyed Ask returning feedback mid-turn, not a verdict.
The structured-result implementation retains that interaction under the selected
conversation's current driver. No decision or route writer survives. Jack has
not selected a replacement for this Ask interaction; bring it to concept review.
