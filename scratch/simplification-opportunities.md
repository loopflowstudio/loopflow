# Simplification Opportunities

Area: how a conversation hears from lf (hooks, plugins, `session.boundary`,
Task updates), LOO-835's surface after PR #1889.

## Product intent

lf adds context and direction to a provider's conversation without getting in
its way. Something new for a conversation (a comment, a run that moved) should
reach it once. It waits for the next boundary unless it needs the agent now.
The Session inbox in `loo-835-connection.md` is that shape; Jack Heart on it:
"this is ok with me".

## Opportunity 1: Task updates become inbox items the server writes

**Misalignment**: the product has one idea, "news for this conversation". The
code has two systems for it. Steers are written by the store
(`task_comments.rs::comment_steer_in`, planning refresh at `stop`), delivered
in `session.boundary` answers and confirmed by batch. Task updates are
computed separately for each conversation by a resident watcher that polls
the store (`harness/task_updates.rs::watch`) and records its own receipts
(`record_task_update`, "dispatched").

**Symptom**:
- `task_updates.rs` (1,358 lines) carries per-observer settling state
  (`candidates` with `Instant`, `poll_again`, `revision`). It has a lock file
  per agent process and thread under `~/.lf/task-updates/`, and a separate
  comment ledger (`TaskObservation.comments`) because comments also arrive as
  steers.
- Four separate pollers run the same computation:
  - Claude: the async `task-updates` hook, `context_block.rs:296,301`
  - Pi: the `watchUpdates` child, `context_pi.mjs:62`
  - OpenCode: the owner's 1 s tick, `lf/commands/util.rs:831`
  - Codex: `Recorder.updates`, `busy` and `update_request` in `codex_connection.rs`
- `ContextMoment::TaskUpdates` and its `PiTaskUpdates` branch in
  `lf/commands/context.rs:257`.
- Seen in practice: 83 watchers holding 5,690 MB on November 11. A comment made
  during a turn arrives twice. This Task session took 25 turns for step
  progress.

**Realignment**: when the store revision for a Task moves, the server works
out what changed once, for each Task, and writes it to each of the Task's
Sessions as a steer, with a source such as `run:<id>` or `position`. The steer
already has `source`, batching and confirmation. Coalescing becomes "replace
the unconfirmed item with the same source", so it's a store rule, not timing
state kept in each observer.

**Cascade**:
- Deleted: the watch loop, its locks, `TaskUpdates` settling, the comment
  ledger, `record_task_update` receipts, `ContextMoment::TaskUpdates`, the async
  hook entries, Pi's child and OpenCode's tick.
- Double delivery can't happen, because a comment is one steer.
- The observer tests in `task_updates.rs` (constant cost under unrelated
  activity, coalescing, reconnect gaps) become store tests of one write path.

## Opportunity 2: one waker per provider, carrying no content

**Misalignment**: once there's one inbox, the only provider-specific job left
is waking an idle conversation. Today each provider's watcher wakes it *and*
delivers the content, each in its own envelope:
- Claude: `cross-session-message` escaping in `claude_updates.rs::deliver`
- OpenCode: a synthetic POST in `opencode.rs::report_task_update`
- Pi: `sendMessage` with `triggerTurn`
- Codex: its own request

So delivery rules (confirmation, attribution, what counts as heard) live in
four places.

**Symptom**: `claude_updates.rs` turns `<lf:task-update>` markup into Claude's
peer envelope. OpenCode's path has to claim `turn_in_progress` itself so it
doesn't race a running turn. Codex holds `busy` for the same reason.

**Realignment**: the server writes a wake marker for a Session only for items
that wake: a person's comment, a run stopping or failing, an interactive step,
a PR published. The waker starts a turn with a fixed short message ("lf has
news"). That turn's first boundary (`UserPromptSubmit` → `session.boundary`)
delivers the content with steer confirmation. Claude's waker is a `/bin/sh`
script blocked on the marker, built the way `context.rs::pi_bash_guard` is.
OpenCode's owner and Codex's observer watch the same marker. Pi's extension
already lives in-process and can watch it with `fs.watch`.

**Cascade**: content delivery happens in one place (`server/boundary.rs`). The
envelope code shrinks to a single constant string per provider. The race
guards go, because a wake that lands during a turn just becomes the next
boundary's steer. A frozen server is no different from an inbox it can't
drain: no marker, nothing wakes, items wait.

Open: Claude's messaging socket token "never leave[s] process memory"
(`claude_updates.rs:2`). A `/bin/sh` waiter launched as the async hook gets the
same env, so the token stays in the hook process the way it does today. Check
that Claude's async hooks still get `CLAUDE_CODE_MESSAGING_*` when a shell
script is the command.

## Opportunity 3: one rule for which comments steer

**Misalignment**: a steer comes only from a *direction* comment
(`linear_observe::is_direction_comment`, `task_comments.rs:288`). A Task
update reports every comment written after launch, except the conversation's
own. People can't tell which comments count as direction. The dedup ledger
in Opportunity 1 exists to bridge the two rules.

**Realignment**: in the inbox, a comment is an item. It wakes the conversation
if a person wrote it, and the conversation's own comments are skipped (rule
kept from `written_here`). If "direction" still means something, it changes
how the item is worded, not whether it's delivered.

**Cascade**: `is_direction_comment` stops gating delivery, and the second
comment path goes away with Opportunity 1. This one needs a person: does a
non-direction comment (a status note, a PR link posted by an integration)
belong in the agent's context at all?

## Aligned areas

- `session.boundary` as the single request every hook and plugin makes, with
  a 2 s deadline and `RecordingGap`. The inbox drains through it unchanged.
- `server_connection.js` shared by Pi and OpenCode, one connection per
  boundary: it holds no slot, and it matches the hook rule.
- Steer confirmation (a batch can arrive twice, never be lost), which the inbox
  inherits as is.
- The managed and bare rule (`WorkSource::is_managed`): bare launches get no
  inbox and no waker.
