# Codex Sessions talk to app-server directly (430c74)

Status: implemented, compressed, synced onto main through #1826, thread following
implemented and compressed (no cuts), realigned 2026-11-05; concept review 2 moved lf's
configuration onto the app-server (implemented and compressed 2026-11-05); concept review 3 found nothing to change (2026-11-05; gate next).
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

## lf's configuration belongs to the private app-server (concept review 2, implemented 2026-11-05)

A Session's app-server is private to it (docs/architecture/execution.md), so lf's
configuration is the server's own, not each thread's. The relay-era "sibling" reason
for per-thread config is deleted.

- Native launch passes shell policy, writable roots, hooks and `developer_instructions`
  as `-c` overrides on `codex app-server`, before the subcommand and after the route's
  and any wrapper's own overrides. lf's thread request carries no `config` and no
  `developerInstructions`. A reconnect joins a server that already has them.
- Writable roots and hooks extend what the server would inherit. lf reads that from a
  short-lived stdio `codex app-server` started with the same command (`config/read`,
  ~0.25 s on 0.166.0), before spawning the real one.
- Hook trust: Codex trusts each handler separately, key `:{group}:{handler}`, hashed
  with `async` (false) and `timeout` (600) defaults filled in (verified with
  `hooks/list`). lf trusted only handler 0, so its async `task-updates` handlers
  needed review; as server config the TUI showed a review dialog. Fixed in
  `codex_hooks`.
- The observer copies the Session's context binding to a `/new` or `/fork` thread
  before selecting it (`context_block::follow_codex_session`).
- Headless Codex keeps its config on `thread/start` (out of scope; its fakes read it).
  Reviewed 2026-11-05: not a second implementation of one rule. A Run has no `/new`, and
  thread config layers over what the server inherits, so headless needs no `config/read`
  probe (~0.25 s per Run). Moving it onto the server would buy only symmetry.

Evidence (codex-cli 0.166.0): a client-started thread with no `config` on an app-server
run with `-c developer_instructions=X` sends X to the Responses server. Real
`codex_terminal.py --new-thread` passes plain and wrapped: the request after `/new`
carries lf's instructions (an `LF_USER_NAME` marker) and its SessionStart hook context.

## Remaining

- Gate: affected suites once (integration `context_launch_tests::`, `session_launch_contract`,
  `session_lifecycle_tests`, `agent_tests`, `settings_launch_tests`, `session_cli_tests`,
  `transport_tests` reach `native_codex.py`; lib `harness::`, `context_block::`).

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
Implement 2026-11-05: `codex_terminal.py --new-thread` (real 0.166.0) plain+wrapped pass;
`--lib harness::` 186 passed; `context_launch_tests::` 13 passed; `session_launch_contract`
26 passed twice (two earlier runs each had two different load-flaky failures, Pi/Claude
included, each passing alone); lifecycle `personal_task_launch_resume...` passed; clippy, fmt clean.
Compress 2026-11-05 (`codex_hooks` returns hooks, `toml_string`): `--lib harness::` 186 passed, `context_launch_tests::codex` 2 passed, clippy clean.
Realign 2026-11-05: code and plan agree on every point of the implement direction; no code change, so the compress checks stand. Wave memory's superseded Codex-limit paragraph replaced.

## Demo

Private `LF_HOME`: start a Codex Session, move `loopflow.db` away, type, get a reply,
run a tool, quit 0. Restore the store; `lf resume` shows history. Kill the observer
while idle: the TUI keeps working.
