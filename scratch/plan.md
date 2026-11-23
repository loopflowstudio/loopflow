# LOO-1002: wake idle OpenCode, Pi and Antigravity work sessions

## Outcome

`lf steer LOO-12 'use the new API'`, or a Task update at a step transition, reaches an
open, idle OpenCode, Pi or Antigravity work session at once: a turn starts in that
terminal with lf's message, as it already does for Claude (cross-session inbox) and Codex
(app-server `turn/start`). Today those three hear it only when someone types. Live case:
MTG-415's Task conversation runs on `agy` and misses the Task updates sent at step
transitions while it sits idle.

Done when the README row "as an idle work session, hears a steer"
(`rust/loopflow-execution/src/harness/README.md:123`) says `yes` for all five harnesses and
`server::wake::tests::` covers each reach.

## Demo

In a Task checkout: `lf task run LOO-x -a opencode`, let the work session finish its
turn and sit idle, then `lf steer LOO-x 'reply with exactly WOKEN'` from another shell.
Within a second the OpenCode TUI starts a turn showing lf's message and answers WOKEN, with
nobody typing into it. Repeat with `-a antigravity` (installed, `agy` 1.5.14) and `-a pi`
(install with `npm install -g --ignore-scripts @earendil-works/pi-coding-agent`; needs a Pi
model login). `lf session steers <id> --json` shows the steer delivered.

## How it works (one model, three new reaches)

The server side stays as `rust/loopflow/src/server/wake.rs` built it: the watcher reads idle
candidates from the store, claims the steers (`steer::claim`, `NativeBoundary::Turn`), sends
them, then calls `record_woken` and `steer::delivered`. What changes is how a conversation
is reached.

**Source of truth:** `agent_sessions.agent_inbox`, "where an idle conversation takes
messages", already written from the boundary's `inbox` field (`server/boundary.rs:109`,
`SqliteStore::record_agent_inbox`). It holds a path per provider:

| provider | `agent_inbox` holds | written by | wake (`IdleReach`) |
|---|---|---|---|
| claude | Claude's peer key file (unchanged) | hook (`claude_inbox()`, `lf/commands/context.rs:500`) | `Claude(key)`: cross-session inbox |
| codex | unused; `agent_endpoint` (unchanged) | launch | `Codex(endpoint)`: `turn/start` |
| opencode, pi | the plugin's wake socket | plugin boundary (`server_connection.js`, today `inbox: null`) | `Plugin(socket)` |
| antigravity | the launch's wake file `{log, token}` (0600) | hook, from an env var lf sets at launch | `Antigravity(file)` |

`idle_conversation_candidates` (`loopflow-storage/src/sqlite/steers.rs:249`) selects
`s.provider` and builds the reach from it, rather than "endpoint means Codex, otherwise
Claude". No migration: every column exists.

### Plugin wake socket (OpenCode and Pi)

Both plugins already live inside the provider process. `server_connection.js` gains a
`wakeSocket(onSteers, log)` that binds `<LF_HOME>/wake/<random>.sock` (directory from the
spec; path under 104 bytes), `unref()`s it, and reports the path as `inbox` on every
boundary. The server connects, writes one line `{"type":"steers","text":…}`, and the plugin
answers `{"type":"accepted"}` once the provider took the message, or
`{"type":"error","message":…}`. A real answer, unlike Claude's inbox, so a send that the
provider refused leaves the claim to lapse. Nothing on the listener throws; a failure is a
`recording gap` log line, and the conversation goes on without it. A launch that reports no
turns (`spec.turns` false: chats, batches) binds nothing.

- **Pi:** `pi.sendMessage({customType: "loopflow-steer", content, display: true},
  {deliverAs: "followUp", triggerTurn: true})`. Pi docs (1.6.0 `docs/extensions.md`, "pi.sendMessage"):
  `triggerTurn` starts a turn when idle; `followUp` waits for a running turn to finish, so a
  wake racing a typed prompt queues instead of interrupting. Since 1.3.0 such a run goes through
  `before_agent_start` like a prompt (CHANGELOG 1.3.0), so context projection is unchanged.
  The existing `report` guard ("outside one it would be stored without starting a run")
  stays: that path is for steers read inside a turn.
- **OpenCode:** `session.synthetic({sessionID: owner, text, delivery: "queue"})` in the plugin's
  `setup` input. Probe on OpenCode 2.0.32 (`opencode serve`, free model): when idle it emitted
  `inbox.enqueued`, `execution.started`, and the model answered; with a turn running, `queue`
  was delivered after it within the same execution. 2.2.1's binary has the same method. Not
  yet watched under the TUI (risk 1). `synthetic`, not `prompt`, so the message is lf's, not
  the person's.
- **OpenCode idle recording:** today the plugin's turn end sends `stop` with `{}`, and
  `boundary.rs:77` records no turn for it ("OpenCode's turn end names no conversation"), so
  an OpenCode work session never shows as idle and the watcher would never find it. The
  plugin's `stop` sends `{session_id: owner, hook_event_name: "Stop"}`, and its `prompt` hook
  reports a `UserPromptSubmit` so a typed turn is recorded as running. That comment goes.

### Antigravity: its language server

`agy` runs its language server in-process, on loopback. Verified by probe on 1.5.14:
launching with `ANTIGRAVITY_SERVER_CSRF_TOKEN=<token>` sets the server's token, and
`POST http://127.0.0.1:<port>/exa.language_server_pb.LanguageServerService/SendUserCascadeMessage`
with header `x-codeium-csrf-token: <token>` and body `{"cascadeId": <conversation>,
"items": [{"text": …}]}` started a turn in an idle TUI: the message rendered as a prompt,
`PreInvocation` and `Stop` hooks fired. The port is in the CLI log line
`Language server listening on random port at N for HTTP`.

For a work session `antigravity.rs` `open()` creates a random token, passes
`--log-file <prompts dir>/<launch>.agy.log`, sets the token in the child's environment, and
writes the wake file `{log, token}` (0600) beside the delivery. Its path rides in an env var
the hook reads into the boundary's `inbox`, as `claude_inbox()` does for Claude. The server reads the
port from the log at wake time (`agy` can move its port across a restart), then posts.
Hooks never see the token (probe: it is absent from hook environments). The agent's own shell
already receives Antigravity's server address and token from `agy`; lf adds nothing there.

The provider thread is already the conversation `/new`, `/fork` and `/resume` moved to
(`hook_callback`), so `cascadeId` follows the person.

## Risks, answered or carried

1. **OpenCode TUI renders a plugin-started turn?** The server path was probed; the TUI subscribes to
   `session.inbox.enqueued`. First slice runs a real `opencode --standalone` work session and
   watches it. If the TUI does not show it, the fallback is `session.prompt` (also probed).
2. **Antigravity's channel is undocumented** (`ANTIGRAVITY_SERVER_CSRF_TOKEN`, an internal RPC).
   An `agy` release can break it. Failure is a logged wake failure and the conversation hears the
   steer at its next boundary, exactly today's behavior. The reach names the `agy` version it was
   seen on in a comment. Sessions launched before this ships have random tokens and stay next-turn-only.
3. **A wake racing a typed prompt.** Pi `followUp`, OpenCode `queue`: both queue behind the running
   turn (OpenCode verified, Pi documented). Antigravity: untested; the server sends only when the
   store says yielded, as for Claude.
4. **Pass-through.** The socket listener and the server's sends are best-effort on both sides. No
   plugin code awaits the server for a wake; a store or server that is down leaves the listener
   idle. Hooks and plugins keep their existing recording-gap paths.
5. **Released lf skew.** Sessions started by the released `lf` report `inbox: null` from plugins and no wake
   file from `agy`: no candidate row, so they keep the next-turn behavior. Nothing reads a new shape.

## Affected surfaces

- `rust/loopflow-storage/src/steer.rs` `IdleReach`: add `Plugin(PathBuf)`, `Antigravity(PathBuf)`.
- `rust/loopflow-storage/src/sqlite/steers.rs` `idle_conversation_candidates`: reach by provider.
- `rust/loopflow/src/server/wake.rs`: two new sends; module doc names all four reaches.
- `rust/loopflow-execution/src/harness/server_connection.js`: wake socket, `inbox` reported.
- `context_pi.mjs`, `context_opencode.js`: accept a wake; OpenCode stop/prompt name the conversation.
- `rust/loopflow-execution/src/harness/context.rs`: spec carries the wake directory.
- `rust/loopflow-execution/src/harness/antigravity.rs`: token, log file, wake file, env var.
- `rust/loopflow/src/lf/commands/context.rs`: hook's `inbox` reports Antigravity's wake file.
- `rust/loopflow/src/server/boundary.rs`: OpenCode Stop records a turn.
- Docs: `server/README.md` "Waking an idle conversation" lists all reaches; harness README row → yes.

Exclusions: chats and batches stay unwoken (unchanged); Antigravity `SendAgentMessage` and the
HTTPS gRPC port are not used; no OpenCode server lf owns (LOO-974's `--standalone` stays).

## Acceptance at gate

```
cargo test -p loopflow --lib server::wake::tests::
cargo test -p loopflow-execution --lib harness::context::tests::
cargo test -p loopflow-execution --lib harness::antigravity::tests::
```

`server::wake::tests::` gains one test per new reach on the existing fixture: an idle Pi or
OpenCode conversation (provider set, `inbox` a test UnixListener that answers `accepted`) takes
one message and its steers are delivered; one that answers `error` keeps its news for the next
boundary; an idle Antigravity conversation (a TcpListener standing in for the language server,
a log naming its port, a wake file) receives the token header and its `cascadeId`. The
`harness::context::tests::` Node runs drive each plugin's wake socket and check Pi's
`sendMessage` options and OpenCode's `synthetic` call.

## Slices

1. **OpenCode end to end** (could prove the plan wrong): `IdleReach::Plugin` by provider in the
   candidate query, the wake socket in `server_connection.js`, OpenCode's `synthetic` call and
   its Stop/prompt naming the conversation, the server send, one wake test. Then a real
   `opencode --standalone` work session in this checkout, idle, `lf steer`: the TUI must show the
   turn (risk 1).
2. **Pi:** the extension's wake handler (`sendMessage` followUp + triggerTurn), Node test, wake test.
3. **Antigravity:** launch token, log file, wake file, hook `inbox`, HTTP send, stand-in test; a
   real `agy` work session woken by `lf steer`.
4. **Docs:** both READMEs; the table row says `yes` and names the tests.

Choices made for review are in `scratch/questions.md`.
