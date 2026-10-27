# LOO-638: context hook follows a native conversation change

Status: implemented on the branch (2026-10-27); gate and demo remain. Jack Heart
filed the Task 2026-10-26 and, by steer on 2026-10-27, brought the resume
target after `/clear` into scope.

## Problem

In an lf-launched Claude Session, every prompt after some point shows
`UserPromptSubmit hook error … invalid control authority: Context callback does
not belong to the selected native conversation`. The prompt still goes through,
but steers and the Loopflow context block never arrive again for the rest of the
process. Jack is the one affected, in any long interactive Session.

## Risk findings (confirmed)

- **Cause, reproduced against real Claude 2.1.311** (PTY probe with a logging
  `--settings` hook, `/clear`, no model turn). One process (hook `$PPID` = the
  Claude pid both times) emits `SessionEnd {session_id: A, reason: "clear"}`,
  then `SessionStart {session_id: B, source: "clear"}`. Every later hook carries B.
- lf's Claude `SessionStart` matchers are `startup|resume` and `compact`
  (`context_block.rs` `hook_settings`), so lf never sees the `clear` start. The
  first callback with B is a `UserPromptSubmit`/`PostToolUse`, and
  `with_provider_context` (`store/sqlite/steers.rs`) refuses it. Two copies
  of "selected thread" both still say A: `steer_adapters.provider_thread`, set at
  first activation, and `agent_sessions.provider_thread`. Even a `SessionStart`
  with B would fail, because `startup` only binds when neither is set.
- A cleared conversation also never gets the start context block. It loses its
  Task brief along with steers.
- **What the thread check exists to fence** (#1535, "reject child and
  replaced-provider callbacks"): Claude subagents (`agent_id` set → `emit_block`
  returns early, unchanged) and a callback naming a different conversation with
  the same capability (`tests/session_steers.rs`, `child-native` on
  `PostToolUse`). Replaced or completed AgentProcesses fail the
  capability/`completed_at` join ("no longer owns this provider"), whatever
  thread they name.
- **Who could present the capability with another thread?** Nested `claude` in a
  Bash tool inherits `LF_PROVIDER_CONTEXT` but gets no lf hooks: they come only
  through the per-launch `--settings` file, and lf never writes project or
  global hooks. Nested lf launches scrub `EXECUTION_IDENTITY_ENV`. Codex
  app-server threads lf did not bind have no delivery file
  (`CONTEXT_DELIVERY_ENV` is removed for the server, see `util.rs`
  `env_remove`), so `load_delivery` fails before the fence. Lf records Codex's
  own thread through the owned transport before the first turn. OpenCode
  `messages.transform` and Pi context fire on every request and OpenCode
  subagent sessions carry no parent ID, so those boundaries **must not**
  reselect.

## Approach

One record says which native conversation an LfSession has selected:
`agent_sessions.provider_thread`. The owned transport writes it (Codex, Claude
stream-json, Pi, OpenCode), and so does a provider's own `SessionStart` declaration
in the owned process. Context callbacks follow it.

1. **Delete the adapter's copy.** Drop `steer_adapters.provider_thread`.
   `reserve_steer_adapter` stops seeding it. `emit_session_steers` matches
   `s.provider_thread=?4` instead of `a.provider_thread`. `steer_batches.provider_thread`
   stays: it is emission history.
2. **Name the boundary instead of `startup: bool`.** `with_provider_context`
   takes a `NativeBoundary` (crate-private enum, `Debug`):
   - `Select`: Claude/Codex `SessionStart` (any source: startup, resume, clear,
     compact). The provider declares the conversation it now runs. Thread
     differs from the selection (or none recorded) → write it as the selection
     and append one `observed` session event
     `{type: "agent_session_selected", agent_process_id, provider_thread: <old>,
     next_provider_thread: <new>, source}`, using the
     `agent_process_replaced` row as the pattern. Same thread → no write.
   - `Activate`: `OpenCodeMessagesTransform`, `PiContext`. Today's startup
     rule: set the selection only when none is recorded, otherwise must match.
   - `Turn`: everything else. Must equal the recorded selection. An empty or
     unset selection is refused, as today.
   `activated_at` keeps meaning "an authenticated callback was observed".
   Under the dispatch fence, the capability + live-AgentProcess join remains the
   process authority, unchanged.
3. **Deliver context to the cleared conversation.** The Claude matcher becomes
   `startup|resume|clear` → `start` moment. `deliver_steers` maps
   `hook_event_name == "SessionStart"` → `Select`. It passes `input["source"]`
   (absent → `"startup"`) for the event payload only, never for authority.
4. Pi (`harness/pi.rs` `with_provider_context(adapter, file, startup, …)`) maps
   its startup to `Activate` and other calls to `Turn`.

Error text for a refused `Turn` gains both ids, so the 2 a.m. log says which
conversation was refused for which: `Context callback names native conversation
<B>; this Session selects <A>`.

## Demo

In an lf-launched Claude terminal Session with a pending `lf session steer`,
type `/clear`, then a prompt. No hook error appears. The fresh conversation
answers with the Task brief in context. `lf session steers <id>` shows the steer
`emitted` on the new thread, and the Session history has an
`agent_session_selected` observation from A to B.

## Alternatives considered

- **Verify hook ancestry** (callback's parent chain reaches the recorded
  AgentProcess pid/birth). Real process fencing, but it depends on Claude's
  hook-spawn shape (sh vs exec) and is a new guard. The capability already
  scopes the process. Rejected (Flexibility). Reconsider only if a provider is
  found running lf hooks in a foreign process with an inherited capability.
- **Rebind on any thread change at any boundary.** Breaks the `child-native`
  fence and lets OpenCode subagent transforms steal the selection. Rejected.
- **Keep both copies, update both on clear.** Leaves the denormalization that
  produced the bug: the owned transport could still move one without the other.
  Rejected.

## Scope

Schema: one draft (`uv run python scripts/new_migration.py
drop_steer_adapter_thread`) with `ALTER TABLE steer_adapters DROP COLUMN
provider_thread`. `steer_adapters` is excluded from `store_revision_*`
triggers (`revisions.rs` non-projection list) and has no index on the column,
so the plain drop applies. Test the released frontier
against the finished draft.

Files: `store/sqlite/steers.rs`, `steers_tests.rs`, `lf/commands/context.rs`,
`context_block.rs` (matcher), `steer.rs` (`NativeBoundary`), `session_record.rs`
(shared `provider_session_observation`), `harness/pi.rs`, `tests/session_steers.rs`,
`docs/architecture-reference.md` Steer row (`steer_adapters` "binds provider
callbacks" → binds the capability to an AgentProcess; the Session's selection
names the conversation).

### Resume target (in scope per Jack's 2026-10-27 steer)

`lf session connect` resumes from `input_provider_session`: the newest
`provider-session:` observation, else the capture's `ProviderSessionObserved`
facts. Terminal Claude has only the launch id there. A `Select` that lands on a
conversation other than the current resume reference appends a new
`provider-session:` observation in the same transaction, carrying the prior
reference's account (LOO-628's account choice survives). A Session with no
resume reference yet leaves publication to its launch transport. Earlier
references stay in history.

### Exclusions

- Codex `/new`-style thread changes through the app-server: the owned
  transport moves the selection; its `SessionStart` now also moves the resume
  reference. No Codex-specific work.
- SessionEnd handling. Not needed: the next SessionStart carries the change.
- Pi `/new` or `/resume` inside one process. `context_pi.mjs` reports only
  `session_start` (mapped to `Activate`); if Pi signals a switch through another
  event, the next context call names a new session file and is refused, the same
  failure for Pi. Unverified against Pi 1.3.0; a separate Task if it reproduces.

## Delete — do not maintain

- `steer_adapters.provider_thread` column and its seeding in
  `reserve_steer_adapter`, the `bound` half of the `with_provider_context`
  comparison, `a.provider_thread` in `emit_session_steers`.
- `startup: bool` parameter on `with_provider_context` (replaced by
  `NativeBoundary`).

Forbidden: keeping the adapter column "for history", a Claude-only branch in the
store, a fallback that silently skips steers on mismatch instead of refusing.

## Done when

Unit (`steers_tests.rs`):
- After `Select` with B on a Session selecting A, `Turn` with B emits a pending
  steer, `Turn` with A is refused, and `agent_sessions.provider_thread = B` with
  one `agent_session_selected` event.
- A second adapter (another AgentProcess claimed through
  `replace_session_agent_process`), still holding the old capability, is refused
  on `Select` with any thread.
- `Activate` with a different thread from the selection is refused (OpenCode
  subagent).

- After `Select`, `input_provider_session` names B on the prior account.

Integration (`tests/session_steers.rs`, existing handoff fixture): send
`SessionStart {session_id: "cleared-native", source: "clear"}` → success, stdout
has the start block with a pending steer. The old conversation's `PostToolUse`
is refused; `PostToolUse` on `cleared-native` delivers the next steer. After the
run, `lf session connect` resumes `cleared-native`; the replaced capability is
refused. The existing `child-native` and `agent_id` cases stay refused.

Gate: `cargo test -p loopflow --lib store::sqlite::steers` and
`cargo test -p loopflow --test integration session_steers < /dev/null`
(inherited `LF_*` unset), migration frontier test, fmt and clippy
`--all-targets -D warnings`. Expected: all pass.

Demo/review (installed lf, real Claude): the Demo above.

## Checks

- 2026-10-27 kickoff probe: real Claude 2.1.311 `/clear` → new `session_id`,
  same pid, `SessionStart source=clear`. Confirmed.
- 2026-10-27 implement: `cargo test -p loopflow --lib store::sqlite::steers`
  (14 pass, incl. draft frontier), `--test integration session_steers` (3 pass),
  clippy clean. Broader gate and installed demo remain.
- 2026-10-27 compress: inlined the `SessionStart` source read, renamed the test
  `ACTIVATE` boundary; `cargo test -p loopflow --lib store::sqlite::steers` 14 pass.
  Deletion list already done; context within budget. Gate owns the rest.
- 2026-10-27 concept review: no change. One record selects (`agent_sessions.provider_thread`);
  the three boundaries map to trust (provider declaration, per-request fill,
  turn). Select writes two history rows for one move (`agent_session_selected`,
  and a `provider-session:` resume reference when one exists); kept, since the
  reference is account-bearing resume evidence and the event also covers
  Activate. Pi switch noted under Exclusions.
- 2026-10-27 realign: plan, code and tests agree (every Done-when case has a
  test); file list corrected. No rerun.
