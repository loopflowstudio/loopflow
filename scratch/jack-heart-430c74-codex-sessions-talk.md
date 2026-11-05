# Codex Sessions talk to app-server directly (430c74)

Status: implemented, compressed, synced onto main through #1826, thread following
implemented and compressed (no cuts), realigned 2026-11-05; concept review 2 moved lf's configuration onto the app-server (next implement).
Jack Heart: lf is a transparent session manager on top of the harness, never a
middleman; Codex is the last harness with lf in the byte path.

## Findings against codex-cli 0.166.0 (private CODEX_HOME, fake Responses server)

- app-server accepts concurrent clients. A client subscribed with
  `thread/resume {threadId}` receives every notification of another client's turn,
  including `item/completed` userMessage text and token usage.
- Server requests (approvals) reach every subscriber; first answer wins and
  `serverRequest/resolved` tells the rest. A silent observer changes nothing.
- A `turn/start` from a second client renders in the real TUI, and its approval
  reaches the TUI.
- The TUI survives the observer's death and keeps taking turns.
- `codex --remote unix://agent.sock resume <id>` joins a running thread; a pending approval raised before it joined reaches it and its answer resolves the turn.
- A started thread has no rollout until its first turn: `thread/resume` fails with
  "no rollout found". `thread/name/set` materialises it. Codex offers nothing more
  direct; if a later version does, replace that one call.

## Job inventory

| Relay job | Direct path |
|---|---|
| Prepared instructions, config, hooks on `thread/start` | lf starts/resumes the thread itself on a short connection, names it, closes |
| Thread identity, context binding, login pin | Launcher, before the TUI exists; the id comes from the reply |
| First input on the client's connection | Observer `turn/start` before the TUI joins. No argv size limit; Codex replays the pending approval to the TUI (verified, 400 KB input) |
| Submission receipts (`codex_submission`) | Deleted. Provider history answers: a thread with turns got its input |
| Turns, usage, availability | `lf __codex-observe`, a second client subscribed to the thread |
| Task updates | Same observer: `turn/start` with `toolOutput` while idle, under the Session fence |
| Reconnect handling | Gone: the TUI reconnects to app-server itself |
| Attachment fence on client writes | Gone by design. The observer ends a TUI whose attachment moved (was ClientWatch) |
| Rejecting `/new`, `/fork` | Gone. See scratch/questions.md |
| Exit settle | Execution owner via `NativeExit`, as Claude; the launcher execs into the TUI |

Store errors in the observer are recording gaps: log, retry the store, keep the
connection. `recover_history` fills gaps from provider history at next launch.

## Process shape

terminal owner (PTY) -> `lf` launcher -> exec `codex --remote <agent.sock> resume ...`
Side: `codex app-server` (own group) and `lf __codex-observe` (own session).
The app-server's lifeline writer crosses the exec, so the TUI holds it and the
server outlives every lf process but the PTY owner. Ending the terminal owner
ends the PTY, as for Claude.

## Delete — do not maintain

Every relay target is gone: `CodexConnection`, relay diagnostics, `client.sock`,
`ClientWatch`, `NativeConnection.relay`, their tests, and `codex_connect.py
--public-connect` with its shared-sibling branches. One relay-era helper stays:
`_public_connection_contract` in `codex_connect.py`, reachable only through
`--planning-peers`, which was already broken on main (scratch/questions.md). It
goes with that fixture's disposition, not this Task.

## The Session follows the conversation (concept review, implemented 2026-11-05)

An LfSession selects one AgentSession at a time; replacement keeps earlier
selections in history, as Claude's `/clear` does through `SessionStart`.
`lf __codex-observe` now does this for Codex `/new` and `/fork`:

- An initialized client receives `thread/started` for threads other clients start
  or fork (verified 0.166.0). On a Session's private app-server lf opens its own
  thread before the observer subscribes, so a later lasting top-level start is the
  person's. Subagents (`parentThreadId` set) and Codex's own helpers
  (`ephemeral: true`; the title writer, `threadSource: "thread_title"`, starts ~2 s
  after the first prompt) are skipped.
- The observer selects the thread through `with_provider_context` with
  `NativeBoundary::Select { source: "new" | "fork" }`: one `agent_session_selected`
  event, the resume reference moves. Then it `thread/resume`s the thread on its
  connection, asking again each tick while Codex has no rollout to resume. The
  resume reply pins the account. A broken store keeps the selection pending.
- `thread/started` no longer reaches the decoder from the observer: its
  `SessionObserved` used to overwrite `provider_thread` with no history, including
  for subagent threads.
- Superseded limit: the plan blamed Codex for the gap: `codex --remote` ignores `-c` for
  its own `thread/start`, so a thread the person starts runs without lf's instructions
  and hooks. The gap is lf's: see the next section.

## lf's configuration belongs to the private app-server (concept review 2, next implement)

lf puts its whole configuration on `thread/start` (`harness/codex.rs` ~L1003-1080:
shell policy, writable roots, hooks, instructions). The reason in the comment at
~L657-666 is that "the app-server can host another conversation" and must not lend
the configuration to a "newly admitted sibling". That was relay-era sharing. Now a
Session's app-server is private to it (docs/architecture/execution.md L179), so every
thread on it belongs to the same Session.

Evidence (codex-cli 0.166.0, private CODEX_HOME): `codex -c developer_instructions="X"
app-server --listen ...` makes `config/read` report `developer_instructions: X`, set
in the `sessionFlags` layer. The current code already reads the wrapper's hooks from
that layer. The defaults for every thread on the server, including one a client starts
without `config`, come from that layer.

Next implement:
- For the native client, pass lf's thread configuration as `-c` flags on `codex
  app-server`, placed before the subcommand with the account route's own flags (the
  comment at ~L1005 notes that a repeat after `app-server` replaces the wrapper's
  overrides). `open_thread` sends no `config` of its own. Delete the "sibling" comment.
- When the observer selects a `/new` or `/fork` thread, it also binds the thread for
  hooks (`bind_codex_session`), as `open_codex_native` does for the first thread.
- Prove it with `codex_terminal.py --new-thread`: a marker in lf's instructions reaches
  the fake Responses server on the person's new thread.
- Before implementing, check that a TUI-started thread's request carries the
  `sessionFlags` instructions. If it doesn't, the Codex limit stands and the old note
  returns, with that evidence.
- Headless Codex is out of scope unless the same move costs nothing there.

This settles open question (b) in scratch/questions.md without a new choice: the
Session follows the thread (accepted), and lf adds context to it (Jack Heart's steer:
"context adder and hook interjector"). The thread gets context from its configuration,
not from an observer interjection, so no turn starts that the person didn't send.

## Remaining

- Implement the section above, then gate: affected suites once.

## Related work

Task e46a80 (Jack Heart, 2026-11-05, in flight) makes the relay this Task deletes
tolerate store errors: ~366 lines in `codex_connection.rs`, plus `native_exit.rs`
and `util.rs`. Whichever lands second meets the conflict; for Codex, this deletion
is the resolution, and e46a80's provider-independent owner-input decoupling stands
on its own. Landing order is Jack Heart's call.

Decided 2026-11-05: retired `--public-connect`. Its headless-to-terminal handoff is
refused by `lf resume` since #1767, and `a_moved_attachment_ends_only_its_own_client`
covers the observer's half.

Checks: real Codex 0.166.0 `codex_terminal.py` plain, `--transport-failure` and
`--new-thread` (plain and wrapped) passed, as did the demo below and `codex_connect.py
--launch --graph-decision-retry replace` after merging main. Two real-Codex fixtures
fail for reasons that predate this Task; see scratch/questions.md.
Realign 2026-11-05: `cargo test -p loopflow --lib -- harness::codex_connection` 6 passed,
`cargo fmt --check` clean; gate owns the affected suites.

## Demo

Private `LF_HOME`: start a Codex Session, move `loopflow.db` away, type, get a reply,
run a tool, quit 0. Restore the store; `lf resume` shows history. Kill the observer
while idle: the TUI keeps working.
