# Codex Sessions talk to app-server directly (430c74)

Status: implemented and compressed, awaiting gate, 2026-11-05.
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
--public-connect` with its shared-sibling branches. No deletion target remains.

## Remaining

- Gate: affected suites once.

Decided 2026-11-05: retired `--public-connect`. Its headless-to-terminal handoff is
refused by `lf resume` since #1767, and `a_moved_attachment_ends_only_its_own_client`
covers the observer's half.

Checks: real Codex 0.166.0 `codex_terminal.py` plain and `--transport-failure` and the
demo below passed before compress; after merging main, `codex_connect.py --launch
--graph-decision-retry replace` passed. Two real-Codex fixtures fail for reasons that
predate this Task; see scratch/questions.md.
Compress after merge: `cargo test -p loopflow --lib harness::codex_connection` 5 passed, clippy clean; gate owns the rest.

## Demo

Private `LF_HOME`: start a Codex Session, move `loopflow.db` away, type, get a reply,
run a tool, quit 0. Restore the store; `lf resume` shows history. Kill the observer
while idle: the TUI keeps working.
