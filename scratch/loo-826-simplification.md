# Simplification Opportunities (after LOO-826, PR #1890)

## Product intent
A managed conversation is registered with the Machine server, and the server
delivers news to it: direction (steers) and progress (Task updates). It delivers
at the next boundary or by waking an idle conversation. lf stays out of the byte
path. A server that is down leaves a recording gap.

## Opportunity 1: one kind of news for a conversation
**Misalignment**: People see a single thing, "what reached this conversation".
The code keeps two ledgers for it. Steers have `steer_revision`,
`emit_session_steers`, prepared/confirmed receipts and `recover_prepared_steers`.
Task updates have `task_update_revision`, `task_updates::read` + `claim` and
`dispatched` receipts. Concept review 1 found a comment delivered twice, once
through each ledger. The fix was a filter ("skip comments whose id has a
steer"), not a merge.
**Symptom**: `session.boundary` answers `steers` and `update` as separate fields
(`server/wire.rs:172,177`). The wake watcher compares a pair of revisions
(`server/wake.rs` `seen: Option<(i64, i64)>`). A wake assembles both
(`boundary.rs::wake_steers` beside `task_updates::claim`). Each hook adapter
prints both. Each has its own tests (`steers_tests.rs`, `task_updates.rs`).
**Realignment**: A Task update becomes an entry in the same per-Session queue
steers use, with a `kind` (direction | progress). It has one revision, one
claim/confirm protocol and one rendered block.
**Cascade**: The comment-dedupe filter goes away. `task_update_revision` and its
triggers, `TaskUpdate`'s separate receipt path, and the boundary answer's second
field all go. Bare conversations get a one-line rule (they skip `progress`).
Needs a migration (expand now, contract after the staging window).

## Opportunity 2: the server records usage limits; delete the last detached `__context-block`
**Misalignment**: Hooks now call the server, but Claude's `StopFailure` hook
still forks a setsid `lf __context-block --moment record-limit`
(`provider_account.rs:1098`, `os_process.rs:240 detached_context_block`). The
fork exists only to outlive the hook's timeout. The server already outlives
every hook.
**Realignment**: The hook sends the limit reason to the server in one request.
A server worker runs `record_limit`, which includes the token refresh that must
not be killed halfway.
**Cascade**: `detached_context_block`, the `record-limit` moment and
`LIMIT_REASON_ENV` are deleted, along with the last per-event lf process a hook
starts. A server that is down falls under the existing `RecordingGap` rule.

## Opportunity 3: Codex history from one source (waits on LOO-822)
**Misalignment**: Codex turns are read back over the app-server RPC
(`codex_connection::recover_history`) from two places: the after-stop worker
(`server/boundary.rs:198`) and native open (`lf/commands/util.rs:1000`, which
starts and stops a harness first). LOO-822 (#1880) reads Session state from
provider transcripts.
**Realignment**: If rollout files carry the turns, transcript reading becomes
the one history source. The server's stop worker and native open both read it.
**Cascade**: `recover_history`, `initialized`, the after-stop worker's
app-server call and the harness start/stop dance in native open are deleted.
`codex_connection.rs` shrinks to `open_thread`, `start_turn` and
`deliver_update` (the wake).

## Aligned areas
- `server/wake.rs`: one watcher, woken by store change events, waking idle
  conversations derived from the store. No per-Session state, no timer except
  `StoreChanges`' 60 s safety net. Keep.
- The Claude inbox reach (`messagingSocketPath` + per-launch
  `crossSessionInbound: accept`, managed launches only) uses the provider's own
  door. lf doesn't sit in the middle.
- `__context-block` as the single hook entry for Claude, OpenCode and Pi.

## Blocked on
- **LOO-821** (#1879, open, Graph running `implement` again): #1890 targets
  `main` and carries LOO-821's commits up to 838e8828c. #1879 has about ten newer
  commits (Desktop Work naming, `resume ID --task`) that aren't in this branch.
  #1890 lands after #1879, then syncs.
- **LOO-822** (#1880, published): decides Opportunity 3 and the brief's open
  question about rollout files.
- **LOO-824** (#1887, published): the PTY holder. Not needed by #1890. It would
  be an alternative way to reach Claude for the wake, and the inbox already
  covers that.
