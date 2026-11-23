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

## Built: the concept review's answers

`direction.md` has the answers word for word.

1. **`agent_inbox` split (5b).** Claude's key stays in `agent_inbox`; the
   OpenCode or Pi socket is in `agent_socket` (`loo974_agent_socket.sql`,
   additive, nothing leaving). The boundary's `inbox` names its kind:
   `{"claude": key}` or `{"socket": path}` (`steer::Inbox`), and
   `IdleReach` is `Codex(endpoint)` or `Inbox(Inbox)`, read from the columns.
   The `provider IN ('opencode','pi')` switch is gone. The wire shape changed
   for Claude too, so a hook from an older `lf` against this server fails its
   boundary (a recording gap) until both are the same build.
2. **A launch's model reaches a reopened OpenCode session (4b).** OpenCode
   2.2.1's plugin `session` has `switchModel({sessionID, model: {providerID,
   id}})`, "the model used by subsequent provider turns". The plugin calls it
   once, in the first `prompt` hook of the launch, with the configuration's
   `model` (`spec.model`). Probe (`probes.md`, "switchModel"): at load and in
   the `prompt` hook both set a reopened session's model, and later turns keep
   it. The `prompt` hook was chosen because it covers workers and new sessions
   without knowing the conversation at load.

Checks, each run once on this tree:

- `server::wake::tests::` 8 passed; `server::boundary::`, `server::schema::tests::`,
  `server::wire::`, `harness::context::tests::` passed after the fixture updates
  (`server_schema.json`, `session_boundary.json`, Swift `ServerProtocolTests`
  edited, Swift suite not run here: gate).
- `harness::context::tests::` and `harness::opencode::tests::`: 18 passed; the
  OpenCode plugin fixture asserts one `switchModel` across two prompts.
- `query_plans::`, `sqlite::steers::`, `sqlite::processes::`: 39 passed.
- `documented_commands::`: 4 passed.
- `tests/e2e/idle_wake.py` on OpenCode 2.2.1 and Pi 1.3.0 (Pi from nvm
  v24.22.2; not on this shell's PATH): passed.

Filed separately as LOO-1068, to run on the `autonomous` Graph once this merges: one owner for "a new provider
process is idle" at its first boundary on every provider (3b), carrying 2a
with it: a reopened work session with steers waiting starts a turn on every
provider. Claude and Codex today claim the steers in their start hook
(server README, "Every launch reads its own steers") and start no turn. The
reason that start hook claims them is not recorded; the new Task finds out
before changing it. Chats keep the start-hook claim: nothing wakes a chat.
LOO-1068 also removes this Task's `observed_at=0` coupling: today
`begin_program_status` and `record_agent_inbox` both move `session_activity`
to a new provider, and the reset trusts a zero reading time to know the status
stream got there first (`scratch/concept-review.html`).

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
- PR #1985 merged on 2026-11-19 (squash) before the idle-wake commits; this
  diff is PR #1988, open. Gate updates it.
- QA on a real model (2026-11-22, `scratch/qa.html`) found both fixes
  below. Every OpenCode model on the account was out of quota, so turns
  named their model and failed; nothing answered.

## Built: a reopen after a failed turn is idle

A reopened work session whose earlier provider ended on a failed turn (no
turn end reported) never woke: the held terminal's status stream
(`begin_program_status`) moved `session_activity` to the new provider before
the plugin named its socket, keeping the old `yielded=0`, so
`record_agent_inbox` skipped its "new provider is idle" reset. The status
stream now zeroes `observed_at` when the provider changes, and the reset also
runs on a reading with `observed_at=0`. Test:
`server::boundary::tests::a_reopened_work_session_is_idle_after_its_terminal_status_moves_first`
(fails without the fix). `server::` 107 passed.

## Built: a woken first turn takes the launch's model

A wake does not pass through OpenCode's `prompt` hook, so a reopened work
session whose first turn was a wake answered on its earlier model. The
plugin's wake handler now calls the same once-per-launch `switchModel`
before `session.synthetic`. The plugin fixture wakes before any prompt and
fails without it. `harness::` 138 passed; real TUI: `model-switched` to the
new model, then the woken turn on it.
- `store::tests::task_facts_update_without_rewriting_work_progression`
  failed once at `seed_task(...).unwrap()` during a 216-test `store::` run on a
  loaded Machine and passed alone; panic message not captured. Untouched by
  this branch.
- Complete LOO-1002 when this merges.
