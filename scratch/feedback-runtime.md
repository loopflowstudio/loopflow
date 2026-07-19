# Next phase: mid-turn Ask

## Problem

A child seeking feedback can only speak at turn boundaries. The feedback step
arms attention when a Turn *ends* (`rearm_feedback_attention`), so asking a
question costs the child its working turn: stop, park, wait for a Steer, start
a new Turn. Steer pushes direction down; nothing pulls an answer up mid-turn.
The counter to Steer is missing.

## Shape

Generalize the tool-response lane. The runtime already answers vendor
permission prompts mid-turn: `permission.asked` → `request_id` →
`ToolResponseWrite { request_id, choice }` fed back into the live turn. An Ask
is the same exchange with a question instead of a permission and the routed
feedback authority instead of an approval policy.

No second lifecycle: an open Ask is a pending tool response on a running
Turn. No new state machine, no queue, no inbox.

### Child side

- An `ask` tool injected into every harnessed client session (same mechanism
  as the vendor permission surface).
- Calling it writes a durable question — `request_id`, question text, current
  Turn, current Basis — through the child's RunLease.
- The write arms the Launch's existing attention columns
  (`attention_kind`/`attention_at`) with the question as payload, so the Ask
  surfaces through the same projections the park path uses: `child_attention`
  for a Parent route, `user_attention` for a User route.
- The tool call blocks. The Turn stays running; the provider holds no tokens
  while a tool result is pending.

### Answer side

- Generalize `ToolResponseWrite { request_id, choice }` to carry text.
- The write is gated by `validate_feedback_caller`: exactly the routed
  authority — the parent Run's lease, or the authenticated User — may answer.
  Same rule as Continue.
- Basis-stamped like every other durable input.
- Delivery: the answer lands as the tool result and the same Turn resumes with
  the answer in hand. The child never lost its context.

### Degradation

If no answer arrives before the harness gives up (turn timeout, process
death), the Turn ends and the open question remains armed attention — exactly
today's turn-boundary park. The Ask lane is an optimization over the same
durable facts, never a second source of truth.

## Decisions to make

- **Answer verb.** Simplest: a pending Ask consumes the next authorized Steer
  as its answer — one input stream, no new CLI surface. Alternative: explicit
  `lf work answer <kind> <id> --request <request_id> <text>`, needed only if
  multiple Asks can be in flight per Launch. Lean single-Ask-at-a-time +
  Steer-as-answer until proven insufficient.
- **Scope.** Available only inside feedback steps, or in any Work with an
  attention route? A background step asking mid-turn is the same mechanics;
  the route already says who answers. Lean: any routed Work.
- **Timeout policy.** Who owns the clock — harness turn timeout only, or a
  bounded Ask deadline that converts to park proactively.

## Not this phase

- Parent-side cadence (keeping the parent's servicing pass dedicated to one
  child until Continue). `start_child_pass` is already shaped for it; design
  it with the project/task server work.
- Independent reviewer bodies (different model/memory conducting review).
  That is parent servicing policy — flow content, not runtime API.

## Done when

- A Task feedback step calls `ask` mid-turn; the parent's `child_attention`
  projection shows the question; an authorized Steer resumes the same child
  Turn with the answer as the tool result.
- An unauthorized answer (User CLI on a Parent route, non-parent Run) is
  rejected with `InvalidAuthority`.
- Killing the answerer before it responds leaves the child parked exactly as
  today: armed attention, no lost state, next Steer starts a new Turn.
- Behavioral tests prove all three; the exchange is visible in `lf trace`.

## Demo

`lf task run <issue> --reviewer parent` on a task whose pursue step asks a
clarifying question mid-turn. Watch `lf trace` on the child: the tool call
opens, the parent's steer arrives as its result, the same turn continues.
Then `lf work continue task <id>` from the parent closes the checkpoint.
