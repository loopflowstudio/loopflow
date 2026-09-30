# Ask and primary conversations: current evidence

Source inspection, 2026-09-30. Supports `scratch/growth-thoughts.md`.
No live Ask was opened, worker launched, or lifecycle test run in this inquiry.

## Current Ask

- `lf/commands/ask.rs` calls `human_session::ask` and returns the completion
  summary. `AskArgs` supports an optional `--skill` and a question.
- `ops/human_session.rs::prepare_ask_record` requires an active Loopflow Run
  manifest, records its parent identity, working directory, Work binding when
  present, model, question, and selected skill. This is available to ordinary
  Runs, not only authored review steps.
- `ask` prepares and launches a separate native Session in that checkout, then
  waits for its completion. `prepare_ask_run` marks it independent of Flow
  membership even when its parent belongs to a Flow.
- `ask_message` permits inspection and editing of the shared checkout. The
  Session agent saves decisions and evidence and marks Ready. `complete_ask`
  requires a ready summary, saves completion before provider cleanup, and releases
  the waiting caller with that summary. Ready and provider exit alone do not
  complete it.
- A raw `ask` allocates a new UUID each call. Internal `ask_once` and
  `task_unblock` reuse a key for an exact execution boundary and retain completed
  feedback for recovery. Raw Ask does not inherit that deduplication just because
  it was raised by the same Task.
- `task_unblock` prepares an `unblock` skill Session for a failed decision.
  Completion supplies evidence for reassessment; it does not choose Advance or
  Iterate. Authored interactive skills are separate Flow Sessions with their
  own recorded boundary.
- The shared `SessionRecord` presents Ask and Flow Sessions to clients. Ask is
  currently a request that creates a conversation and waits, not an asynchronous
  FYI message routed into an existing primary conversation.

## Agent guidance

`engine/builtins/surfaces/headless.md` instructs agents to make safe executive
decisions, use Ask when progress genuinely requires a decision, and otherwise
record material assumptions and continue. `surfaces/human-present.md` says to
ask in the existing conversation rather than create another Session.

`task/skill/unblock.md` can resolve a narrow question or use concept-review for
deeper reconsideration. It forbids completing on the participant's behalf or
choosing Flow navigation. `task/skill/loop-decide.md` requires reassessment after
Ask and warns against automatically opening identical Asks.

Thus capability is already broad; intended use is constrained by the need for
judgment. “Raise a chat whenever desired” would broaden the interaction policy.

## Existing continuity

`controller/wave/README.md` describes a durable Wave channel with identity,
memory, and journal, separate from its governance scheduler. The resident
process survives across turns, but `controller/wave/chat_reply.rs::reply_prepared`
creates a harness, runs one reply, and stops it. A persistent runtime is not
evidence of one persistent provider conversation.

`wave/skill/wave_chat.md` supplies Wave identity/memory and recent messages,
defaults to silence, and launches work only when asked. It is not currently
specified as a general agent receiving every Task's Ask. Ordinary native Sessions
also support continuation, but neither mechanism establishes cross-Task Ask
routing or one primary conversational owner.

## Proof limits and design pressure

Further inspection for Jack's persistent AgentSession proposal found no exact
`AgentSession` or `agent_session` identifier in Rust/Swift sources.
`run_record.rs::ProviderSessionRef` retains provider session/account identity;
`human_session.rs::open` uses recorded provider history and `resume_session` for
ordinary interactive Sessions. This supports reuse of existing continuation
mechanisms, but does not establish scope-owned primary identity or Ask routing.

The existing `complete_ask` lifecycle resolves a request and stops its native
Session. This can fit Jack's clarified direct Task Ask UX. Only requests handled
inside a persistent primary conversation would need resolution independent of
conversation completion. The Wave resident's governance scheduler and the
listener's transport/journal responsibilities are separate migration concerns.

`swift/LoopflowMac/WorkspaceProjection.swift` currently groups Task Sessions by
their explicit Work binding and the Task runtime's Work ID. It does not match
worktrees in that projection. `SessionFlowMembership` is a separate execution
property. Jack requires Task worktree grouping even for independently launched
Sessions; this needs presentation/association work, not inferred Flow membership.

Existing simulated tests cover keyed Ask reuse, independent boundaries, retained
completion, and completion surviving cleanup failure. These tests were read,
not rerun; they do not prove the configured desktop experience.

Opening a new conversation for every question risks making Jack coordinate the
workers. Routing everything through one primary agent risks a bottleneck and
loss of detailed evidence. The design must preserve the originating question,
exact waiting work, and explicit answer even if its presentation changes.
