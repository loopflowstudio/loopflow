# Ask and primary conversations: current evidence

Source inspection reconciled at `841f3c580`, 2026-09-30. Supports
`scratch/growth-thoughts.md`. Executed local proof and its limits are recorded in
`kickoff-evidence.md`; no configured live Ask is established by this evidence.

## Current Ask

- `lf/commands/ask.rs` calls `human_session::ask_with_key` and returns the completion
  summary. `AskArgs` supports optional `--skill` and `--key` arguments and a question.
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
- A raw `ask` without `--key` allocates a new UUID each call. An explicit key is
  scoped to the originating Run; a retry joins the pending Session or returns its
  retained answer. The same key in another Run is a separate question. Internal
  `ask_once` and `task_unblock` reuse a key for an exact execution boundary and
  retain completed feedback for recovery. Raw Ask does not inherit their boundary
  identity just because it was raised by the same Task.
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

The earlier inspection described the Wave resident, `chat_reply::reply_prepared`,
and `wave_chat.md`. Upstream `3dc89bc9a` deleted those paths, the listener and
external chat bridge. They are historical evidence only. The surviving
`controller/wave/journal.rs` contains a short-ID formatter, not turn ownership.
Ordinary native Sessions still support continuation; this does not establish
primary scope identity, automatic turn delivery, or cross-Task Ask routing.

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
conversation completion. Automatic wakes need a new consumer of the surviving
observation outbox under the primary execution owner; the removed listener's
claim/requeue behavior is no longer an available migration mechanism.

`swift/LoopflowMac/WorkspaceProjection.swift` now groups Task Sessions using
Rust-derived `workspace.taskId` and the Task runtime's Work ID. The first internal
cut supplies checkout association independently of Run attribution and
`SessionFlowMembership`. Home/worktree-keyed pane retention and reassociation are
now implemented in the window-owned registry. Focused Swift tests support surface
retention; the configured cross-Task/provider demonstration remains unexecuted.

Recorded simulated tests cover raw caller-scoped keys, concurrent retries,
independent boundaries, retained completion, and completion surviving cleanup
failure. They do not prove the configured desktop experience.

Opening a new conversation for every question risks making Jack coordinate the
workers. Routing everything through one primary agent risks a bottleneck and
loss of detailed evidence. The design must preserve the originating question,
exact waiting work, and explicit answer even if its presentation changes.
