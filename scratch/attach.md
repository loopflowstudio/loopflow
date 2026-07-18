# The Human Door

## Architecture dependency

Build this on the architecture branch's target model:

```text
Work -> Epoch -> Run -> Launch -> optional Turn
                  \-> Wait
```

Do not recreate `InteractionReview`, `InteractiveHandoff`, `Block`, a generic
inbox, directives, or incorporation acknowledgements. Review is already a
projection of the current interactive flow position, its live Launch, its
Basis, and `attention: User | Parent(WorkRef)`.

The implementation branch should stack on or rebase onto architecture after
that branch's Run-authority cut is coherent. It should not bridge the old and
new models or preserve compatibility with the deleted review and handoff
tables.

## Problem

The architecture has a clean runtime meaning for "this Work needs a
conversation," but the user-facing door is still process-shaped. `lf task
attach` and generic Launch presentation expose tmux or a provider process when
the product action is simpler: see what needs you, talk to the agent already
conducting the interactive step, and continue the Work.

INT-10 is the live specimen. Its kickoff reached a human review boundary, but
the human had no obvious product surface for continuing that conversation.
Headless execution and sidecar prose filled the gap. The architecture now has
the facts that gap was missing; this change gives them a human surface.

## The demo

Blue light on a Task in Concerto -> tap -> embedded Ghostty opens its current
Review as a conversation -> human and agent exchange ordinary turns -> human
chooses Continue -> the interactive flow step closes and Work continues.

Closing the window only closes the presentation. The Review stays blue and can
be reopened. No process name, attach command, cleanup ritual, outcome form, or
approval disposition appears.

CLI first:

```text
lf queue
lf work review task <task>
lf work review task <task> --continue-on-success
lf work review task <task> --continue-on-exit
```

`lf queue` lists current User-routed Reviews. `lf work review` opens the same
conversation client Concerto embeds. Bare CLI defaults to explicit Continue;
Concerto launches with `--continue-on-exit`.

## Existing nouns

- **Interactive flow step** -- authoring-time declaration that the step is a
  conversation. The step's skill is the skill being conducted; there is no
  separate pause-point record or default-skill field.
- **Review** -- runtime projection of `FlowPosition + Launch + Basis +
  AttentionRoute`. It has no stored id, prompt, status, reviewer, disposition,
  or terminal outcome.
- **User attention queue** -- an oldest-first query over Reviews whose route is
  `User`. It is a view, not a stored queue.
- **Steer** -- one authored contribution to the conversation. Human input is
  recorded as `Author::User`; a parent response is authored by the active
  parent Run.
- **Turn** -- the agent's observed response at an immutable Basis.
- **Close** -- the explicit, Basis-fenced action that advances the interactive
  flow step and clears attention.

## Key decisions

### 1. The queue is a projection, not a new model

The machine-wide query returns every current Review with
`attention == User`, ordered by `opened_at` and stable identity. A queue item
needs enough projection data to render and open the conversation:

```rust
struct UserReviewProjection {
    review: Review,
    surface: LaunchSurface,
    latest_output: Option<String>,
    evidence: serde_json::Value,
}
```

This is a DTO/query result, never a row. `WorkRef` is the durable address;
`LaunchId` identifies the current presentation route; `Basis` fences each
Review Steer and close. If recovery replaces the Run or Launch, reopening by
Work finds the new projection.

Parent-routed Reviews never appear in `lf queue`. The parent control lane
already derives and services those Reviews before background work.

### 2. Conversation belongs to the current Work agent

Opening a Review does not launch a reviewer, fork a provider session, or run a
second skill. The interactive skill is already the current flow step in the
Work's active Launch.

The terminal client:

1. reads the current Review, latest root Turn output, and current Work evidence;
2. records each human line as a User Steer;
3. displays later Turns from the Work's current Launch;
4. refreshes the Review projection when recovery replaces a Launch;
5. applies the selected Continue-on-exit policy when the client ends.

The first version renders complete root Turn output. Live token spectating is a
separate transport feature and is not required for the conversational model.

The provider's resume token remains private Launch continuity. Losing it may
make the next Turn less fluent, but cannot lose the Steers, Basis, flow
position, or current Work evidence needed to reconstruct the conversation.

### 3. Humans use Steer without seeing a control protocol

The terminal is a thin presentation client over the architecture's existing
User authority:

- typed text -> `steer_review_if_current(WorkRef, LaunchId, text, if_basis)`;
- agent response <- current Turn root output;
- `/continue` or the Concerto Continue control ->
  `close_review(WorkRef, if_basis)`;
- client termination -> the selected explicit, success, or exit policy.

The human never types `lf work steer` or handles Basis values. The client does
that translation. A stale-Basis rejection leaves the changed Review open and
never silently retries direction against a newer conversation. The worker
receives the same ordered durable Steer whether it came from this client or
from the active parent Run.

There is no implicit "human engaged, no conclusion" Steer. A weaker exit
policy represents absence of a conclusion by leaving the Review open; a
stronger policy explicitly chooses continuation without inventing direction.

The product verb is **Continue**, not Done. It says only that this conversation
has supplied enough direction for the flow to advance; it does not assert that
the human approved the work or that the Work itself is complete. `close_review`
remains the internal domain operation.

### 4. Review is declared by the flow, not the skill

Review behavior never comes from skill frontmatter. A skill can run headlessly
in one flow and conversationally in another; the flow step owns that choice.
`interactive` may still select the presentation of a skill invoked directly,
but it does not create or route a Review.

Conceptually:

```yaml
- step:
    name: review-design
    review: true
```

The lifecycle's interaction policy chooses only the attention route:

- `require` -> `AttentionRoute::User`;
- `defer` -> `AttentionRoute::Parent(immediate_parent)`.

Both routes conduct the same skill in the same child Work Launch through
Steers and Turns.

Parent-to-User escalation is a direct, narrow control:

```text
escalate_review(child_work, if_basis)
```

Only the active Run of the immediate parent currently named by
`AttentionRoute::Parent` may call it. The transaction verifies the Review and
Basis are still current, changes the same Launch's attention to `User`, and
sets the User-attention timestamp used for queue ordering. It does not author a
Steer, start another Turn, copy a transcript, or create another Review. A stale
Basis, a different parent, or an already User-routed Review is rejected.

### 5. tmux is containment, not the human API

Retire `lf task attach` from the product surface. A provider-backed Review is
opened through the Work conversation client, never by entering the worker's
stdin or tmux session.

Architecture may still use tmux as Launch containment or as the attach route
for an opaque TUI Launch. `lf launch present` remains the low-level door for
that distinct case. It does not become Review identity, status, or recovery
truth.

### 6. Exit policy is explicit

The conversation client has three mutually exclusive policies:

```text
lf work review task <task>                         # explicit
lf work review task <task> --continue-on-success  # clean completion
lf work review task <task> --continue-on-exit     # any exit
```

- **explicit** -- only `/continue` advances the Review. Ctrl-C, EOF, error,
  `/detach`, and window close leave it open.
- **success** -- a normally completed client advances before returning success.
  `/detach`, an internal error, a signal, or a crash leaves it open.
- **exit** -- every client end advances: normal return, EOF, Ctrl-C, nonzero
  error, signal, window close, SIGKILL, or app crash. This mode has no detach
  override; choose a weaker policy when leaving the Review open must remain
  possible.

`--continue-on-success` and `--continue-on-exit` conflict; the latter subsumes
the former. Concerto uses `--continue-on-exit`, while a terminal invocation
without either flag remains parked until explicit Continue.

Clean completion calls `close_review` before the process returns. Any-exit
semantics cannot rely on an exit handler, so that mode starts a detached exit
supervisor in its own process session. The client owns the write end of a pipe;
normal return, error, signal, SIGKILL, terminal close, or app death closes it.
The supervisor commits each complete client Steer itself, then attempts one
narrow `continue_review_if_current` transition when the pipe closes. A process
death cannot land between the Steer's commit and the supervisor learning its
new Basis.

The supervisor is presentation machinery, not Review identity or Run authority:

- it holds only the current Work, Launch, and latest Basis the client displayed
  or authored;
- it accepts only complete Steer messages from its owning client and fences
  each write by the exact User-attention Review and Basis;
- a concurrent unseen Steer, changed flow position, changed attention route,
  or replacement Launch makes the exit continuation stale and harmless;
- a process lock permits only one any-exit client for a Review at a time;
- recovery opens a new client against the replacement Launch; the old
  supervisor cannot advance it.

No lfd control API, keeper, Review row, lease row, outcome, disposition, or
implicit Steer is added. The supervisor cannot author Run input or steer Work
outside the exact User Review it guards.

Provider Launch death remains separate: containment and recovery evidence
change, then a recovered Launch resumes the same interactive flow position.

### 7. Blue is derived User attention

Blue means a current Review routes attention to the User. It is neither generic
waiting nor failure. Red remains broken/recovery-required; green remains
advancing; unknown remains unreadable evidence; black remains off and clean.

The single-lens fold is:

```text
red > blue > green > unknown > black
```

Blue propagates from Task to Project and Wave so the invitation is visible
outside the detail view. The queue remains complete even when a red sibling
wins the aggregate lens.

Tapping a blue Task launches `lf work review` in embedded Ghostty. Remote Work
uses the existing Home/SSH launch routing so the client executes against the
owning store; no lfd control API is introduced.

## Terminal behavior

The client presents one conversation, not a shell:

```text
INT-10 · review-design
Waiting for you

Agent
I have mapped the boundary around provider evidence. The remaining choice is
whether inferred identity may ever satisfy a proof obligation.

You
No. Unknown remains unknown; only provider evidence may close the gap.

Agent
Understood. I updated the design around that invariant. Anything else?

/continue
Continuing Work
```

Useful local commands are presentation controls, not new domain operations:

- `/continue` -- advance at the latest displayed Basis;
- `/status` -- render current Work, step, Launch, and attention;
- `/detach` -- exit while leaving Review open in explicit or success mode;
  unavailable with `--continue-on-exit`.

If the Review closes elsewhere, the client reports that the step advanced and
exits. If recovery changes Launches, it reports the transition and exits so a
new client can open the replacement presentation route. If no Review currently
exists, it refuses to invent one.

## Done when

- `rg 'InteractionReview|InteractiveHandoff|BlockId'` has no production
  references;
- no Review, queue item, transcript, disposition, or outcome row is added;
- one Review contains multiple User Steers and agent Turns without changing
  identity;
- a parent and a human produce the same child-visible Steer shape;
- the current parent can escalate its child Review directly to User attention;
  stale and non-parent escalation is rejected;
- only a current Work, Launch, and Basis continuation advances the Review step;
- every human surface labels that action Continue, never Done or Approve;
- default CLI exit leaves Review and attention unchanged;
- success mode continues only after normal client completion;
- exit mode continues after clean exit, error, signal, SIGKILL, or app death;
- a stale exit supervisor cannot advance a changed Review;
- recovery can replace a Launch without losing the Review's Work/flow meaning;
- User attention appears in `lf queue` and as blue in Concerto; parent attention
  appears in neither;
- the queue and Concerto consume the same Rust-owned projection;
- provider-backed Review never requires tmux attachment;
- remote and local Work open through the same product action;
- skill metadata no longer decides whether a flow pauses for conversation;
- docs describe Review through Work, Steer, Turn, Basis, and attention only.
