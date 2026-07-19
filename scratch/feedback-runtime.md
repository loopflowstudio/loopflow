# Finish Work and make Feedback small

## Status

This design finishes the independent part of the Session-to-Work migration and
defines the smaller Feedback architecture. It deliberately stops before the
Home/Work server topology. That follow-up must choose process ownership before
runtime dispatch, liveness, retries, streaming, or stopped-parent wake behavior
is moved.

The work is split into six reviewable slices:

1. remove implicit PR review state;
2. delete Radio and channel identity;
3. make `MEMORY.md` the only Wave memory;
4. remove ambient recent Wave chat from prompts;
5. finish Project/Task planning nouns and delete copied parent state;
6. contract Feedback presentation, reviewer, receipt, and Session APIs.

## Intent

There should be one durable control model for Wave, Project, and Task, with
different domain policy on top:

```text
Work       stable Wave, Project, or Task identity
Steer      durable authored input to Work
Run        one period of execution authority
Launch     one provider/process attempt inside a Run
Turn       one observed provider boundary
Wait       the exact fact that can make another Run useful
Feedback   the current authored Task flow checkpoint, when one is open
```

Task Feedback is scheduled by explicit `feedback: true` flow markers. The
reviewer is either the User or the immediate parent Project. In unattended
operation the immediate parent performs review; it is not an automated actor
masquerading as the User.

Project questions and Wave judgment are ordinary on-demand Work, not another
Feedback protocol. Escalation is not part of Task Feedback. A Task can choose
the User directly as reviewer, and hierarchy-wide escalation—when needed—is
one hop at a time: Task to Project, Project to Wave, Wave to the human through
its human surface.

## Current runtime truth

The things called servers do not yet form one runtime:

| Process | Current job | Missing ownership |
| --- | --- | --- |
| Home resident | Hosts in-process Wave listeners and answers health/start | Does not scan Ready Work, dispatch Project/Task, or reconcile reserved Runs |
| Wave listener | HTTP/SSE, journal, conversation, observations, resident supervision | Uses one coarse lifetime Run and does not consume generic Work Steers |
| Wave resident | Cadence, playhead, provider harness, live chat | Records Wave-specific journal lifecycle instead of generic Launches/Turns |
| Project runner | Ephemeral tmux body that supervises Tasks and judges KRs | Launch, wake, liveness, and recovery depend on callers |
| Task runner | Ephemeral tmux body that owns flow, workspace, PR, and CI policy | Duplicates Project runtime controls and often exits without a typed Wait |

A “Project server” does not currently exist. A stopped Project cannot answer
Task Feedback merely because the durable route points at it; no common owner is
responsible for noticing useful input and starting the Project.

The server follow-up must decide:

1. whether one Home-local process hosts lightweight Work actors or supervises
   per-Work processes;
2. whether Wave cadence is a long-lived actor outside bounded Runs;
3. whether Feedback ends a Task Run in `Wait` or keeps an idle presentation
   process without making that process truth;
4. which live deltas the Mac app needs and which durable Turn/Trace projection
   is sufficient after reconnect;
5. how remote Home nudges work while correctness still comes from Ready scans;
6. whether Wave conversation becomes generic Work steer/follow and whether the
   Wave journal can then disappear.

That follow-up is done when it can draw one process diagram and name exactly one
owner each for dispatch, liveness, retry, event streaming, and remote nudge.

## Feedback protocol

Feedback belongs to the authored Task flow position, not to a Launch or a
separate review object. Its durable information is the current Work, Basis,
flow position, and reviewer:

```rust
enum FeedbackReviewer {
    User,
    Parent,
}

struct FlowFeedback {
    reviewer: FeedbackReviewer,
    opened_at: OffsetDateTime,
}
```

The parent target is derived from `task.project_id`; it is never copied into a
route id. Evidence is read live from current Work, PR, CI, and the deliberately
selected child Turn. There is no Feedback id, disposition, copied evidence
JSON, escalation state, or exit guard.

Opening Feedback records the checkpoint and makes the reviewer useful. Closing
Feedback is one explicit `work continue` transaction fenced by Basis and
authority. Presentation never advances flow. Closing a terminal, losing tmux,
provider exit, success, error, signal, crash, Home restart, or Launch loss must
leave the same checkpoint visible.

PR state is not Feedback. `OpenPr` is a URL affordance. A merged PR either
continues the Task's serial PR chain or completes the Task, as selected by the
operator. There is no implicit post-merge Review state.

## Communication and continuity

Work Steer is the one durable authored input. Chat is a human presentation of
Steers and Turns, not another mailbox, identity graph, or history truth. Radio
and channel identity are deleted.

Continuity already exists in domain structure:

```text
Wave    -> GOAL.md + MEMORY.md + Project portfolio
Project -> definition + proof-shaped KRs + Task set
Task    -> directive + worktree + serial PR chain
Work    -> current Basis + unconsumed Steers
```

Recent conversation is not a context-selection rule. A Turn enters a prompt
only when an operation deliberately selects it. Project and Task prompts do not
inherit the latest Wave transcript.

Wave memory is exactly `wave/<name>/MEMORY.md`, read from applicable ancestor
Waves oldest-first. It has no journal delta stream, receipt, replay buffer,
HTTP write route, SSE event, or compaction skill. `lf memory show` may remain as
a direct read-only convenience and must work while every server is stopped.

## Planning truth

Session tables are gone. Project and Task should expose their domain truth
without execution-shaped wrappers:

```rust
struct ProjectDefinition {
    id: LinearProjectId,
    slug: String,
    name: String,
    prompt_context: String,
    pm_snapshot_synced_at: i64,
}

struct TaskDirective {
    id: LinearIssueId,
    identifier: String,
    title: String,
    description: String,
    pm_snapshot_synced_at: i64,
}

struct Task {
    directive: TaskDirective,
    project_id: ProjectId,
}
```

Task does not copy Project definition. Prompt, status, roadmap, and diagnostic
paths resolve the parent when they need its data. Updating a Project therefore
changes the next Task prompt without rewriting every Task.

Project/Task `agent`, `provider`, `provider_session_id`, abandonment intent, and
handoff remain temporarily because they bridge Run reservation to Launch,
fallback, resume, and shutdown. Removing them requires the server follow-up to
choose durable route ownership and generic stop/recovery.

## Public API after the independent contraction

```text
lf project start | run | status | promote | existing specialized controls
lf task start | run --reviewer user|parent | status | changes | diff | file
lf work status | steer | feedback | continue | interrupt | abandon
lf launch list | status | attach | present | handback
lf memory show
```

Omitting `--reviewer` keeps the standard Task plan:

```text
kickoff -> user
iterate -> parent
gate    -> user
```

An explicit reviewer changes future checkpoints only. It never changes provider
presentation and never skips an authored skill.

Project/Task generic control aliases remain until the Work host exists because
they currently reconcile liveness, start Runs, wake stopped bodies, observe PR
state, and perform provider handoff. Deleting them first would strand Steers and
reproduce the stopped-Project bug.

## Deletion and shrink ledger

### Delete now

- `ReviewGateState`, `TaskAction::Review`, `AfterMerge::Review`;
- Radio CLI, bus store/schema/runtime, channel family, `LF_CHANNEL`, machine
  bylines, `MessageOp::Say`;
- live memory events/state/routes/SSE/Swift DTOs and `export-memory`;
- ambient Wave chat prompt field, gatherer, renderer, budgets, and tag;
- `ProjectLaunchReceipt`, `TaskLaunchReceipt`, Linear snapshot wrappers, and
  Task's copied Project snapshot;
- Feedback exit-continuation flags, guard process, retry/lock state, and
  conditional continuation store method;
- Feedback escalation command/store transition;
- `InteractionPolicy`, `FlowAction` feedback policy, `--headless` reviewer
  overload, and persisted require/defer vocabulary;
- producerless evidence `Receipt`, resolver CLI, evidence kinds, and PR receipt
  identity helpers;
- Work/Launch wire fields and UI types that call stable identity Session.

### Defer to server design

- `agent_launches.attention_*`, `AttentionRoute`, `Feedback`, `ChildFeedback`,
  `UserFeedback`, and Launch-attention Swift DTOs;
- Project/Task duplicated steer/resume/wait/interrupt/attach/abandon behavior;
- `ops::child`, `ChildBodyHandoff`, domain provider route/resume/abandon fields;
- Home Ready scan, generic Work status DTO, stopped-Work wake, Wave generic
  Launch/Turn conversion, and chat-to-steer conversion;
- exact fate of Wave listener/runtime/resident/supervisor and journal.

## Behaviors unlocked

- A merged PR continues unless the operator explicitly completes the Task.
- Presentation lifecycle cannot advance authored flow.
- Direct User review and immediate-parent review share one checkpoint protocol.
- Memory reads and prompt assembly work without a live Wave.
- Project updates become visible to Tasks without denormalized rewrites.
- Radio expiry, forged bylines, channel prefixes, and recent transcript order
  cannot change product state or prompt behavior.
- The server follow-up begins from one vocabulary instead of preserving Session,
  interaction, review, feedback, chat, radio, and memory variants of the same
  control ideas.

## Impossible by design

- PR publication or merge invents a Review gate.
- Closing or crashing a presentation advances Feedback.
- A Feedback checkpoint changes reviewer after opening.
- A sibling, grandparent, forged channel, or stale Run answers parent Feedback.
- Launch loss deletes the authored checkpoint.
- Recent Wave discussion silently enters an unrelated Project or Task prompt.
- Radio traffic, bus retention, or a dotted channel changes product truth.
- `MEMORY.md` disagrees with a live delta log.
- Task carries a Project identity that disagrees with `project_id`.
- Evidence receipts survive with no producer.
- Provider session continuity is mistaken for Work identity.

## Implementation slices

### 1. PR state

Rename Review action/disposition to `OpenPr` and `ContinueTask`, migrate stored
`review` to `continue_task`, delete review-gate branches, and prove merged
continuation.

### 2. Radio and channels

Delete bus/channel modules, latest-schema tables, CLI commands, listener wiring,
machine bylines, docs, prompts, and compatibility spellings.

### 3. File-only memory

Delete live memory journal/runtime/server/SSE/Swift state and write commands.
Keep only direct file read and ancestor prompt assembly.

### 4. Explicit prompt context

Delete ambient Wave transcript injection. Preserve Wave chat as a human product
surface pending server design.

### 5. Planning nouns

Flatten planning wrappers into Project definition and Task directive, remove
Task's copied parent, and resolve the parent deliberately.

### 6. API contraction

Make Feedback presentation-only, delete escalation, rename reviewer routing,
delete orphan receipts, and rename Work/Launch identities still called Session.

## Done when

### PR state

- [x] `ReviewGateState`, `TaskAction::Review`, and `AfterMerge::Review` are absent.
- [x] `OpenPr` is presentational only.
- [x] Stored `after_merge` accepts only `continue_task|complete_task`.
- [x] Merged `ContinueTask` work proceeds without an approval record.

### Communication and memory

- [x] Radio/channel modules, CLI, store methods, bus tables, `LF_CHANNEL`,
      `MessageOp::Say`, and machine bylines are absent.
- [x] Project promotion and builtin prompts use typed evidence, not Radio.
- [x] Prompt memory reads only applicable `MEMORY.md` files.
- [x] Memory journal variants, replay, runtime state, HTTP writes, SSE frames,
      Swift facts, write commands, receipts, and export skill are absent.
- [x] `lf memory show` works without a server.
- [ ] `PromptComponents` has no Wave chat field and generated prompts contain no
      `<lf:wave-chat-recent>`.
- [ ] Project/Task prompt tests prove unrelated recent Wave Turns are absent.

### Planning data

- [ ] Project exposes `definition`; Task exposes `directive` and `project_id`.
- [ ] LaunchReceipt/snapshot wrappers and Task's copied Project data are absent.
- [ ] Task SQL projects no parent PM metadata into Task.
- [ ] Project definition updates change the next Task prompt/status without a
      Task rewrite.

### Feedback and API safety

- [ ] `lf work feedback` only presents and `lf work continue` is the only close.
- [ ] Exit guard, continuation flags, and escalation API/store state are absent.
- [ ] Task help exposes `--reviewer user|parent`, with mixed default and explicit
      override behavior proved.
- [ ] Reviewer columns/values use reviewer and user/parent; no dual reader.
- [ ] `InteractionPolicy` and dead feedback `FlowAction` machinery are absent.
- [ ] Evidence Receipt models, command, resolver, docs, and PR helpers are absent.
- [ ] Work/Launch wire identity uses task/work/launch rather than session.
- [ ] Product help/docs contain no Task Session, Project Session, or durable
      Session identity; remaining Session terms name real provider/terminal/time
      sessions or explicitly deferred `provider_session_id` fields.

### Static proofs

```bash
rg -n 'ReviewGateState|TaskAction::Review|AfterMerge::Review' rust/loopflow/src swift
rg -n 'LF_CHANNEL|AmbientChannelRef|ChannelRole|BusListener|MessageOp::Say|lf radio' rust/loopflow/src swift docs --glob '!rust/loopflow/src/store/migrations/**'
rg -n 'MemoryAdded|MemoryUpdated|MemoryFact|memory-add|lf memory (add|log|update)' rust/loopflow/src swift docs
rg -n 'wave-chat-recent|gather_wave_chat|render_wave_chat|wave_chat' rust/loopflow/src swift docs
rg -n 'ProjectLaunchReceipt|TaskLaunchReceipt|LinearProjectSnapshot|LinearIssueSnapshot|launch_context|\.launch\.(project|issue)' rust/loopflow/src rust/loopflow/tests
rg -n 'continue_on_success|continue_on_exit|FeedbackExitGuard|FeedbackExitPolicy|WorkCommand::Escalate|escalate_feedback|FeedbackEscalated' rust/loopflow/src swift docs
rg -n 'InteractionPolicy|WaitFeedback|DeferFeedback|next_action_with_policy' rust/loopflow/src
rg -n 'ReceiptCommand|EvidenceKind|ResolvedReceipt|lf receipt|pr_identity' rust/loopflow/src swift docs
```

Historical migrations may name the schema they migrate. Current source has no
compatibility parser, alias, dual reader, deprecated DTO, or hidden command for
the deleted APIs.

## Server-design handoff

After the six slices, remaining unchecked items should all be topology-bound:

- Home owns Ready scanning, dispatch, retries, and reconciliation;
- Wave uses bounded Runs/Launches/Turns and generic Steers;
- Feedback moves off Launch attention onto flow position;
- specialized controls collapse into generic Work operations;
- Project/Task executor fields and handoff disappear;
- Swift consumes one Work snapshot/Feedback shape;
- Wave chat becomes generic steer/follow or is removed as a separate API.

If a remaining item can be completed without choosing one of those owners, it
belongs in these six slices and is not deferred.
