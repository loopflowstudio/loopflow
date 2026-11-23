# OpenCode attached-worker probe, 2026-11-19

Real OpenCode CLIs, isolated home and repository, and a scripted local
model API. The server was private and authenticated. The fixture sent a
text answer, a question, or a shell tool that wrote a start marker, waited
six seconds and wrote a finish marker.

| Observation | 2.0.32 | 2.2.1 |
|---|---|---|
| Attached JSON client answered and could run a second turn on the same session | yes | yes |
| Native transcript stored 31 input and 7 output tokens for the fixture answer | yes | yes |
| Attached `--auto` emitted a cancelled question as a tool error, then continued and exited 0 if left alone | yes | yes |
| SIGTERM to the attached client returned 130 but left the private server up and the tool wrote its finish marker | yes | yes |
| Stopping the private server as well prevented the finish marker | yes | yes |
| SIGTERM to the standalone client prevented the finish marker | yes | yes |

On 2.2.1, stopping only the attached client on the question error still
allowed the model's next shell tool to run. Stopping the server at that
line prevented the follow-up request in the probe. The standalone client
stop prevented it too. These runs show the ownership difference. They do
not establish how fast the provider can start another tool after a question.

A disposable Git-backed Task used the branch `lf` with a temporary CLI
wrapper that replaced `run --standalone` by `run --server` and passed the
launch's plugin configuration to a private 2.2.1 server. The model received
the Task context and a steer sent while the shell tool ran. The step reached
its interactive review. History had the fixture answer, a native thread
and 62 input / 14 output tokens for the completed worker. The commit-message
helper received the same fixture response and reported no commit message.
The wrapper tested the worker transport through a Task. The design since
chosen keeps `run --standalone` and starts no OpenCode server (`plan.md`).

# Idle wake from inside the provider, 2026-11-19

Real TUIs under a PTY, isolated home, scripted local model. After the first
turn settled, a file appeared and the plugin or extension, on a timer,
asked the provider to start a turn with `WAKE_MARKER from lf`. Nothing was
typed. Scripts: `probes/oc_wake.py`, `probes/pi_wake.py`.

| Observation | OpenCode 2.0.32 | OpenCode 2.2.1 | Pi 1.3.0 |
|---|---|---|---|
| A turn started in the idle session and the model request carried the text as a user message | yes | yes | yes |
| The answer rendered in the TUI | yes | yes | yes |
| Typed input afterwards ran a normal turn | yes | yes | yes |

OpenCode: `session.synthetic({sessionID, text})` and
`session.prompt({sessionID, text})` both started the turn and returned
`delivery: "steer"`. `prompt` drew the text in the TUI as a user message;
`synthetic` drew nothing for it, only the answer. The plugin's `session`
also has `interrupt` and `wait`. Pi: `pi.sendMessage(..., {triggerTurn:
true})` and `pi.sendUserMessage(text)` both started the turn and drew the
text.

Not tested: a wake racing a typed prompt, and a wake while a permission or
question dialog is open. The real `lf` path has since run end to end
(`tests/e2e/idle_wake.py`, OpenCode 2.2.1 and Pi 1.3.0).

# `codex exec --json` as a worker, 2026-11-19

codex-cli 0.170.0, isolated `CODEX_HOME`, scripted local Responses API.
Script: `probes/codex_exec.py`.

| Observation | Result |
|---|---|
| Stream carries thread id, tool start and finish, final answer, usage | yes |
| `codex exec resume <id> --json` continues the exact thread | yes |
| SessionStart, PostToolUse and Stop hooks fire, given through `-c hooks={…}` | yes |
| PostToolUse `additionalContext` reaches the next model request | yes |
| SIGTERM to the process group during a tool | exit -15; the tool kept running and wrote its finish marker; rollout has no abort record |
| SIGINT during a tool | exit 1; finish marker never written; rollout records `turn_aborted`, `reason: interrupted` |
| The JSON stream says the turn was interrupted | no |
| Usage limit (429 `usage_limit_reached`) | exit 1; stream has `error` and `turn.failed` with message text; the typed `usage_limit_exceeded` is only in the rollout |

`exec` prints config warnings as `item.completed` errors before
`turn.started`. Hooks ran with `--dangerously-bypass-hook-trust`;
Loopflow's trusted hashes were not exercised. Of about 2,350 non-test
lines in the four Codex files, roughly 300–450 serve only the headless
driver; an `exec` decoder with rollout reads would add about as many.

# Configured model in the OpenCode TUI, 2026-11-19

OpenCode 2.2.1 TUI under a PTY, isolated home, scripted local API with three
models (`neon`, `carbon`, `small` as `small_model`). Each row is one launch of
`opencode --standalone --session <id> --prompt …` with `model` set in
`OPENCODE_CONFIG_CONTENT` as shown. Script: `probes/oc_model.py`.

| Launch | Model the prompt's request named |
|---|---|
| new session, config `opencode/carbon` | `carbon` |
| same session reopened, config `opencode/neon` | `carbon` |
| same session reopened, no config `model` | `carbon` |
| another new session in the same home, config `opencode/neon` | `neon` |

A config `model` picks the model of a new session. A reopened session keeps
the model of its last turn and ignores the config. No `model.json` state file
was written; the model rides with the session.

# switchModel from the OpenCode plugin, 2026-11-22

OpenCode 2.2.1 TUI under a PTY, scripted local API, config `model`
`opencode/neon` on every launch. A probe plugin called
`session.switchModel({sessionID, model: {providerID, id}})` once per launch,
at load (`setup`) or in its first `prompt` hook. Script: `probes/oc_switch.py`.

| Launch | Models the prompts' requests named |
|---|---|
| new session | p1: `neon` |
| reopened, switch to carbon at load, two prompts | p2: `carbon`; p3: `carbon` |
| reopened, no switch | p4: `carbon` |
| another new session, `prompt`-hook switch to carbon | p5: `carbon` |
| that session reopened, `prompt`-hook switch to neon | p6: `neon` |

Both calls succeeded. The switched model holds for later turns and later
launches until something switches it again.
