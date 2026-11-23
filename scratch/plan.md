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

QA on real models (2026-11-19, private Machine, Gemini through OpenCode and
Pi, `scratch/qa.html`) added two things:

- A work session quit and reopened could not be woken until someone typed:
  OpenCode's plugin named its socket only at a turn, and Pi's activity row
  stayed on the earlier process. The plugin now names its socket as it loads,
  and the first boundary naming a new inbox records the provider as idle and
  runs the wake (`boundary::After`, `record_agent_inbox`).
- `idle_wake.py` left its holder and provider running after every run.

Checks after those: `server::` 106 passed (after `boundary::After` became an enum),
`harness::context::tests::` 13 passed, both `idle_wake.py` runs passed.

## Built: the configured model reaches a new work session

An OpenCode work session ignored the model in `agent:`: the 2.2.1 TUI has no
`--model` flag, so `opencode:opencode/gemini-4-carbon` ran the account's
default. `opencode::environment` now sets `"model"` in
`OPENCODE_CONFIG_CONTENT`, and `opencode run` no longer passes `--model`, so
workers and work sessions take it from one place.

What the real TUI showed (`probes.md`, "Configured model"): the config picks
the model of a new session; a reopened session keeps the model of its last
turn. So changing `agent:` between opens of one OpenCode work session does
not change its model. Stopped there, per the slice; `questions.md` 7.

Checks: `cargo test -p loopflow-execution --lib harness::opencode::tests::`:
5 passed, 1 ignored (live). `scratch/probes/oc_model.py` on 2.2.1: as above.

QA on a real model (2026-11-19, `scratch/qa.html`): a new work session and a
worker both ran `gemini-4-carbon` from `agent:` (account default: neon); an
idle steer and a steer saved while quit both woke it after the compress; a
changed `agent:` on reopen stayed on carbon. `idle_wake.py` passed on
OpenCode 2.2.1 and Pi 1.3.0.

## Next slice: from the concept review's answers

`direction.md` has the answers word for word. Two changes here; the
reopen rule goes to its own Task.

1. **Split `agent_inbox` (5b).** Claude's key file stays in `agent_inbox`;
   the OpenCode or Pi socket goes in a new `agent_socket` column
   (`src/migrations/loo974_agent_socket.sql`, created empty, additive). The
   boundary request names which it carries, so `IdleReach` reads its arm from
   the column and the `provider IN ('opencode','pi')` switch in
   `idle_conversation_candidates` goes. Mark nothing leaving: the column
   keeps its Claude meaning. Regenerate the DTO fixture if the wire changes.
2. **A launch's model reaches a reopened OpenCode session (4b).** The plugin
   sets the model from `agent:` on the first turn of each launch only, then
   leaves it. That is what `claude --resume --model` does, and it keeps a
   `/models` choice made inside the TUI, which a set-every-turn plugin would
   undo each turn (the cost of 4b as asked). Probe first which plugin hook
   can change a turn's model on 2.2.1 (`chat.message`, `chat.params`); if
   none can, stop and record it in `questions.md` 7 rather than restarting
   the conversation (4c).

Then QA: reopen an OpenCode work session after changing `agent:`; the first
turn answers on the new model, and a `/models` pick after it holds.

Filed separately as LOO-1068, to run on the `autonomous` Graph once this merges: one owner for "a new provider
process is idle" at its first boundary on every provider (3b), carrying 2a
with it: a reopened work session with steers waiting starts a turn on every
provider. Claude and Codex today claim the steers in their start hook
(server README, "Every launch reads its own steers") and start no turn. The
reason that start hook claims them is not recorded; the new Task finds out
before changing it. Chats keep the start-hook claim: nothing wakes a chat.

## Remaining
- A provider killed with SIGKILL leaves its empty `/tmp/lf-wake-*` directory.
  Accepted: empty, and cleared on reboot.

- Not tested: a wake while a permission or question dialog is open. A wake
  racing a typed prompt ran on real OpenCode: the steer's turn, then the typed
  one, each once.
- OpenCode 2.0.32 has no end-to-end run of the wake (the probe covered its
  `session.synthetic`): `idle_wake.py` on it stops at the first turn with
  "Reconnect OpenCode Console to continue", the fixture's `opencode` provider
  config never reaching the scripted model. `command -v opencode` is 2.0.32 on
  this Machine; 2.2.1 is `/opt/homebrew/bin/opencode`.
- PR #1985 merged on 2026-11-19 at 18:46 UTC, before the idle-wake commits.
  It was a squash, and the branch has no open PR: gate publishes the
  idle-wake diff (`git diff 2a3b6d11d8 HEAD`) as a new one.
- Complete LOO-1002 when this merges.
