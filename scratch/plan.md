# LOO-974 harness design, and the slice it leaves to build

`direction.md` holds the priorities this was chosen against. `probes.md`
holds what the real CLIs did. The plan and QA for what PR #1985 already
contains are at `52e3f97ffd:scratch/plan.md`.

## The design

The provider's own CLI runs the conversation. Loopflow owns that process's
lifetime and adds to it only through code the provider loads: a hook, a
plugin or an extension. Loopflow starts a provider server only where the
provider offers no other way in.

| Job | Rule | Claude | Codex | OpenCode | Pi |
|---|---|---|---|---|---|
| Worker or batch turn | one provider process per turn; ending it stops the model and its tools | `claude -p` stream | app-server and Rust driver | `opencode run --standalone --format json --auto` | `pi --mode rpc` |
| Context and mid-step steer | the hook runs `lf __context-block` at start and after each tool | hook | hook | plugin | extension |
| Work session | native TUI in the held terminal; reopening resumes the thread by id | TUI | `codex --remote` on its private app-server | `opencode --standalone` | TUI |
| Steer to an idle work session | the Machine server uses the provider's own way in, never the byte path | inbox socket | app-server turn start | **plugin starts the turn** | **extension starts the turn** |

No Loopflow-owned OpenCode server, for work sessions or workers. Its two
uses were rejoin and idle wake. Rejoin is a resume by thread id, as on
Claude. Idle wake works from inside the plugin (`probes.md`), with no
second process per conversation.

Codex keeps its app-server for work sessions and for workers. `codex exec
--json` can run a worker, but stop needs SIGINT (a kill orphans the tool),
interrupt and usage-limit status are only in the rollout file, and the
change is about neutral in lines while leaving two Codex decoders. A GUI
chat would also want the app-server driver.

## Built

The fourth row is closed: `lf steer` to an idle OpenCode or Pi work session
starts a turn. `IdleReach::Extension`, the socket in `server_connection.js`
(`wakeSocket`), and the third arm in `server::wake` are in, with the two
README cells at "yes".

What the slice taught:

- OpenCode's native TUI recorded no turns at all, so the server could never
  see it idle. A work session's plugin now reports the turn's first request
  (`UserPromptSubmit`) and names its conversation at the turn end (`Stop`).
  A turn that fails or is interrupted reports no end, as on Claude: the
  steer then waits for the next turn.
- The socket answers. The server sends `{"thread","text"}`, closes its side
  and waits for `started`; a plugin on another conversation, or a provider
  that throws, closes without it and the steers stay undelivered.
- A held terminal reads the account's provider files, not the launching
  shell's `OPENCODE_CONFIG_CONTENT` or `PI_CODING_AGENT_DIR`.
- `provider` on `agent_sessions` picks the reach: `opencode` and `pi` are
  the socket, anything else with an inbox is Claude's key.

Checks, each run once on this tree:

- `cargo test -p loopflow --lib server::wake::tests::`: 8 passed.
- `cargo test -p loopflow-execution --lib harness::context::tests::`: 13 passed.
- `cargo test -p loopflow --lib server::schema::tests::`: 8 passed
  (`tests/fixtures/dto/server_schema.json` regenerated for the `inbox` doc).
- `cargo test -p loopflow --test integration documented_commands::`: 4 passed.
- `tests/e2e/idle_wake.py --harness opencode` (2.2.1) and `--harness pi`
  (1.3.0): passed by hand; gate and QA rerun them where the CLIs exist.

## Remaining

- Not tested: a wake racing a typed prompt, and a wake while a permission or
  question dialog is open. Both providers say they queue the message as a
  steer; only the Node stand-ins and the idle case were run.
- OpenCode 2.0.32 was not run end to end here (the probe covered its
  `session.synthetic`).
- PR #1985 merged on 2026-11-19 at 18:46 UTC, before the idle-wake commits.
  The branch is three commits ahead of `origin/main` with no open PR, so
  gate publishes them as a new one.
- Complete LOO-1002 when this merges.
